//! Pure policy for Area's logical-window visibility transaction. No native
//! pointers or OS calls live here; the caller owns synchronous borrowed handles.
use bello_platform::macos_capture_overlay::OwnedWindowContext;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Finish preflight before returning any mutation/restore plan. A hidden or
/// minimized owned window is valid but never enters the prior-visible set.
pub(super) fn preflight<H: Copy, E>(
    handles: &[H],
    mut inspect: impl FnMut(H) -> Result<bool, E>,
) -> Result<Vec<H>, E> {
    let checked = handles
        .iter()
        .copied()
        .map(|handle| inspect(handle).map(|visible| (handle, visible)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(checked
        .into_iter()
        .filter_map(|(h, visible)| visible.then_some(h))
        .collect())
}

pub(super) struct AreaTransaction<H> {
    pub id: u64,
    pub generation: u64,
    pub requester: H,
    /// Recorded BEFORE mutation: orderOut can succeed before validation fails.
    pub restore: Vec<H>,
    pub selector: Option<H>,
    pub cancellation: Arc<AtomicBool>,
    workers: usize,
    finishing: bool,
    frozen: bool,
}
impl<H: Copy + Eq> AreaTransaction<H> {
    pub fn new(id: u64, generation: u64, requester: H, restore: Vec<H>) -> Self {
        Self {
            id,
            generation,
            requester,
            restore,
            selector: None,
            cancellation: Arc::new(AtomicBool::new(false)),
            workers: 0,
            finishing: false,
            frozen: false,
        }
    }
    pub fn cancelled(&self) -> bool {
        self.cancellation.load(Ordering::Acquire)
    }
    pub fn cancel(&mut self) {
        self.cancellation.store(true, Ordering::Release);
        self.finishing = true;
    }
    pub fn finish(&mut self) {
        self.finishing = true;
    }
    pub fn start_worker(&mut self) {
        self.workers += 1;
    }
    pub fn finish_worker(&mut self) {
        assert!(self.workers > 0, "unbalanced Area worker completion");
        self.workers -= 1;
    }
    pub fn accept_freeze(&mut self) -> bool {
        if self.cancelled() || self.frozen {
            return false;
        }
        self.frozen = true;
        true
    }
    pub fn allows(
        &self,
        generation: u64,
        requester_live: bool,
        context: OwnedWindowContext,
        topology_matches: bool,
    ) -> bool {
        !self.cancelled()
            && requester_live
            && topology_matches
            && context.allows_capture_continuation(self.generation, generation)
    }
    pub fn restore_live(&self, live: &[H]) -> Vec<H> {
        self.restore
            .iter()
            .copied()
            .filter(|handle| live.contains(handle))
            .collect()
    }
    /// Restoring promptly on cancel does not release the global capture lock
    /// while a worker or a retiring selector still owns heavy image resources.
    pub fn can_release(&self) -> bool {
        self.finishing && self.workers == 0 && self.selector.is_none() && self.restore.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bello_platform::macos_capture_overlay::OwnedKeyWindow;
    fn context() -> OwnedWindowContext {
        OwnedWindowContext {
            application_active: true,
            application_hidden: false,
            requester_visible: false,
            key_window: OwnedKeyWindow::NoKeyWindow,
        }
    }
    fn tx() -> AreaTransaction<u64> {
        AreaTransaction::new(1, 4, 10, vec![10, 20])
    }
    #[test]
    fn failed_preflight_never_exposes_a_partial_mutation_plan() {
        let mut inspected = Vec::new();
        let result = preflight(&[10, 20, 30], |h| {
            inspected.push(h);
            if h == 30 {
                Err("invalid/sheet/fullscreen")
            } else {
                Ok(true)
            }
        });
        assert_eq!(inspected, [10, 20, 30]);
        assert!(result.is_err());
    }
    #[test]
    fn partial_hide_failure_keeps_failed_and_not_yet_mutated_prior_visible_handles() {
        let plan = preflight(&[10, 20, 30], |_| Ok::<_, ()>(true)).unwrap();
        let transaction = AreaTransaction::new(1, 4, 10, plan);
        let mut mutated = Vec::new();
        for handle in &transaction.restore {
            mutated.push(*handle); // Native mutation precedes validation failure.
            if *handle == 20 {
                break;
            }
        }
        assert_eq!(mutated, [10, 20]);
        assert_eq!(transaction.restore_live(&[10, 20, 30]), [10, 20, 30]);
    }
    #[test]
    fn restoration_excludes_closed_new_prior_hidden_and_minimized_windows() {
        let plan = preflight(&[10, 20, 30, 40], |h| Ok::<_, ()>(h == 10 || h == 20)).unwrap();
        let transaction = AreaTransaction::new(1, 4, 10, plan);
        assert_eq!(transaction.restore_live(&[20, 30, 40, 50]), [20]);
    }
    #[test]
    fn stale_navigation_closed_requester_and_cancel_suppress_completion() {
        let mut transaction = tx();
        assert!(transaction.allows(4, true, context(), true));
        assert!(!transaction.allows(5, true, context(), true));
        assert!(!transaction.allows(4, false, context(), true));
        transaction.cancel();
        assert!(!transaction.allows(4, true, context(), true));
        assert!(!transaction.accept_freeze());
    }
    #[test]
    fn application_focus_and_topology_must_match_even_when_key_survives() {
        let transaction = tx();
        for changed in [
            OwnedWindowContext {
                application_active: false,
                ..context()
            },
            OwnedWindowContext {
                application_hidden: true,
                ..context()
            },
            OwnedWindowContext {
                key_window: OwnedKeyWindow::OtherWindow,
                ..context()
            },
        ] {
            assert!(!transaction.allows(4, true, changed, true));
        }
        assert!(!transaction.allows(4, true, context(), false));
    }
    #[test]
    fn cancellation_restores_promptly_but_stays_busy_until_worker_cleanup() {
        let mut transaction = tx();
        transaction.start_worker();
        transaction.selector = Some(99);
        transaction.cancel();
        assert_eq!(transaction.restore_live(&[10, 20]), [10, 20]);
        transaction.restore.clear();
        transaction.selector = None;
        assert!(!transaction.can_release());
        transaction.finish_worker();
        assert!(transaction.can_release());
    }
    #[test]
    fn navigation_cannot_release_busy_before_visible_selector_retirement() {
        let mut transaction = tx();
        transaction.selector = Some(99);
        transaction.cancel();
        transaction.restore.clear();
        assert!(!transaction.can_release());
        transaction.selector = None;
        assert!(transaction.can_release());
    }
    #[test]
    fn freeze_acceptance_is_single_use_and_success_still_requires_cleanup() {
        let mut transaction = tx();
        assert!(transaction.accept_freeze());
        assert!(!transaction.accept_freeze());
        transaction.finish();
        assert!(!transaction.can_release());
        transaction.restore.clear();
        assert!(transaction.can_release());
    }
}
