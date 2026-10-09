//! One bounded JSON calculation owner, retained across palette-to-window transfer.
//! A host owns views; this entity owns the draft, generation and physical worker.
use crate::session::{JobToken, SessionJobs};
use gpui::Context;
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Pretty,
    Minify,
    Validate,
}
impl Mode {
    pub const ALL: [Self; 3] = [Self::Pretty, Self::Minify, Self::Validate];
    pub fn label(self) -> &'static str {
        match self {
            Self::Pretty => "Pretty",
            Self::Minify => "Minify",
            Self::Validate => "Validate",
        }
    }
    pub fn argument(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Minify => "minify",
            Self::Validate => "validate",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "minify" => Self::Minify,
            "validate" => Self::Validate,
            _ => Self::Pretty,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Palette,
    Window,
    Retired,
}

pub(crate) struct JsonSession {
    input: Arc<str>,
    pub mode: Mode,
    pub output: String,
    pub error: Option<String>,
    pub status: String,
    pub busy: bool,
    owner: Owner,
    jobs: SessionJobs,
    desired: Option<JobToken>,
    running: bool,
    invalid_input: bool,
    #[cfg(test)]
    probe: WorkerProbe,
}
impl JsonSession {
    pub fn new(input: String, preview: bool, cx: &mut Context<Self>) -> Self {
        let mut session = Self {
            input: Arc::from(""),
            mode: Mode::default(),
            output: String::new(),
            error: None,
            status: String::new(),
            busy: false,
            owner: if preview {
                Owner::Palette
            } else {
                Owner::Window
            },
            jobs: SessionJobs::default(),
            desired: None,
            running: false,
            invalid_input: false,
            #[cfg(test)]
            probe: WorkerProbe::default(),
        };
        session.set_input(input, cx);
        session
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn oversized_preview(&self) -> bool {
        self.owner == Owner::Palette && self.input.len() > bellobox_core::MAX_PREVIEW_BYTES
    }
    pub fn can_copy(&self) -> bool {
        self.owner != Owner::Retired
            && !self.busy
            && self.error.is_none()
            && !self.output.is_empty()
    }
    pub fn can_chain(&self) -> bool {
        self.can_copy()
            && self.mode != Mode::Validate
            && self.output != self.input.as_ref()
            && self.output.len() <= bellobox_core::MAX_INPUT_BYTES
    }
    pub fn chain(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.can_chain() {
            return false;
        }
        self.set_input(self.output.clone(), cx);
        true
    }
    pub fn set_draft(&mut self, input: String, mode: Mode, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        if !self.invalid_input && self.input() == input && self.mode == mode {
            return;
        }
        self.mode = mode;
        self.set_input(input, cx);
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.owner != Owner::Retired {
            self.changed(cx);
        }
    }
    pub fn set_input(&mut self, input: String, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        if let Err(error) = bellobox_core::validate_input(&input) {
            self.reject_input(error, self.mode, cx);
            return;
        }
        self.invalid_input = false;
        self.input = Arc::from(input);
        self.changed(cx);
    }
    /// Validate at the editor boundary before cloning an oversized user draft.
    pub fn reject_input(&mut self, error: String, mode: Mode, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        self.mode = mode;
        self.jobs.cancel();
        self.desired = None;
        self.busy = false;
        self.invalid_input = true;
        self.output.clear();
        self.error = Some(error);
        self.status.clear();
        cx.notify();
    }
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired || self.mode == mode {
            return;
        }
        self.mode = mode;
        self.changed(cx);
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.jobs.cancel();
        self.desired = None;
        self.busy = false;
        self.output.clear();
        self.error = None;
        self.status = "Cancelled.".into();
        cx.notify();
    }
    pub fn retire_palette(&mut self, cx: &mut Context<Self>) {
        if self.owner == Owner::Palette {
            self.retire(cx);
        }
    }
    pub fn retire_window(&mut self, cx: &mut Context<Self>) {
        if self.owner == Owner::Window {
            self.retire(cx);
        }
    }
    pub fn retire(&mut self, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        self.cancel(cx);
        self.owner = Owner::Retired;
    }
    pub fn can_transfer(&self) -> bool {
        self.owner == Owner::Palette && !self.invalid_input
    }
    /// Called only after successful destination creation. The job is unchanged.
    pub fn transfer(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if !self.can_transfer() {
            return Err("This JSON session is no longer in the palette.".into());
        }
        let deferred = self.oversized_preview();
        self.owner = Owner::Window;
        if deferred || (!self.busy && self.output.is_empty() && self.error.is_none()) {
            self.changed(cx);
        } else {
            cx.notify();
        }
        Ok(())
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        let token = self.jobs.begin();
        self.output.clear();
        self.error = None;
        self.status.clear();
        self.desired = None;
        self.busy = false;
        if self.invalid_input {
            self.error = Some(format!(
                "Input exceeds {} UTF-8 bytes; it has not been truncated.",
                bellobox_core::MAX_INPUT_BYTES
            ));
            cx.notify();
            return;
        }
        if crate::tool_controls::source_input_is_idle(&self.input) || self.oversized_preview() {
            cx.notify();
            return;
        }
        self.desired = Some(token);
        self.busy = true;
        self.status = "Working locally…".into();
        self.start_worker(cx);
        cx.notify();
    }
    fn start_worker(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(token)) = this.update(cx, |s, _| s.desired) else {
                    let _ = this.update(cx, |s, _| s.running = false);
                    break;
                };
                cx.background_executor()
                    .timer(Duration::from_millis(220))
                    .await;
                let request = this.update(cx, |s, _| {
                    (s.desired == Some(token) && s.jobs.accepts(token))
                        .then(|| (s.input.clone(), s.mode))
                });
                let (input, mode) = match request {
                    Ok(Some(request)) => request,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                #[cfg(test)]
                let probe = match this.update(cx, |s, _| s.probe.clone()) {
                    Ok(p) => p,
                    Err(_) => break,
                };
                #[cfg(test)]
                let executor = cx.background_executor().clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        #[cfg(test)]
                        {
                            probe
                                .starts
                                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            while probe.held.load(std::sync::atomic::Ordering::SeqCst) {
                                executor.timer(Duration::from_millis(10)).await;
                            }
                        }
                        crate::execute("json", &input, mode.argument())
                    })
                    .await;
                if this
                    .update(cx, |s, cx| s.publish(token, result, cx))
                    .is_err()
                {
                    break;
                }
                // At most one physical calculation per session. Edits only
                // replace desired state; the next iteration takes the latest.
            }
        })
        .detach();
    }
    fn publish(&mut self, token: JobToken, result: Result<String, String>, cx: &mut Context<Self>) {
        if self.desired != Some(token) || !self.jobs.accepts(token) || self.owner == Owner::Retired
        {
            return;
        }
        self.desired = None;
        self.busy = false;
        match result {
            Ok(output) => {
                self.output = output;
                self.status = "Valid JSON · processed locally".into();
            }
            Err(error) => {
                self.error = Some(error);
                self.status = "This operation needs attention.".into();
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct WorkerProbe {
    pub held: Arc<std::sync::atomic::AtomicBool>,
    pub starts: Arc<std::sync::atomic::AtomicUsize>,
}
#[cfg(test)]
impl JsonSession {
    pub(crate) fn worker_probe(&self) -> WorkerProbe {
        self.probe.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};
    #[gpui::test]
    fn transfer_keeps_job_and_cancelled_or_stale_results_never_publish(cx: &mut TestAppContext) {
        let session = cx.new(|cx| JsonSession::new("{\"x\":1}".into(), true, cx));
        session.update(cx, |s, cx| {
            let first = s.desired.unwrap();
            s.transfer(cx).unwrap();
            assert_eq!(s.desired, Some(first));
            s.publish(first, Ok("first".into()), cx);
            assert_eq!(s.output, "first");
            assert!(s.transfer(cx).is_err());
            s.set_input("{\"x\":2}".into(), cx);
            let next = s.desired.unwrap();
            s.publish(first, Err("late".into()), cx);
            assert!(s.error.is_none());
            assert!(s.output.is_empty());
            s.cancel(cx);
            s.publish(next, Ok("late".into()), cx);
            assert!(s.output.is_empty());
            assert!(!s.can_copy());
            s.retire(cx);
            s.set_input("{}".into(), cx);
            assert!(s.desired.is_none());
        });
    }
    #[gpui::test]
    fn modes_chain_bounds_and_independent_sessions(cx: &mut TestAppContext) {
        let a = cx.new(|cx| JsonSession::new("{}".into(), true, cx));
        let b = cx.new(|cx| JsonSession::new("[]".into(), true, cx));
        a.update(cx, |s, cx| {
            s.set_mode(Mode::Validate, cx);
            let token = s.desired.unwrap();
            s.publish(token, Ok("Valid JSON".into()), cx);
            assert!(s.can_copy());
            assert!(!s.chain(cx));
            assert_eq!(s.input(), "{}");
            s.set_mode(Mode::Minify, cx);
            s.publish(token, Ok("late".into()), cx);
            assert!(s.output.is_empty());
            s.publish(s.desired.unwrap(), Ok("[1]".into()), cx);
            assert!(s.chain(cx));
            assert_eq!(s.input(), "[1]");
            s.set_input(" ".repeat(bellobox_core::MAX_PREVIEW_BYTES + 1), cx);
            assert!(s.oversized_preview());
            assert!(s.desired.is_none());
            s.transfer(cx).unwrap();
            assert!(!s.oversized_preview());
            s.set_input("x".repeat(bellobox_core::MAX_INPUT_BYTES + 1), cx);
            assert!(s.error.is_some());
            assert!(s.desired.is_none());
            assert!(!s.can_copy());
        });
        b.update(cx, |s, _| {
            assert_eq!(s.input(), "[]");
            assert_eq!(s.mode, Mode::Pretty);
            assert!(s.busy);
        });
    }
    fn tick(cx: &mut TestAppContext, ms: u64) {
        cx.run_until_parked();
        cx.background_executor
            .advance_clock(Duration::from_millis(ms));
        cx.run_until_parked();
    }
    #[cfg(feature = "developer-tools")]
    #[gpui::test]
    fn physical_worker_coalesces_edits_and_survives_transfer(cx: &mut TestAppContext) {
        use std::sync::atomic::Ordering::SeqCst;
        let s = cx.new(|cx| JsonSession::new("{\"n\":1}".into(), true, cx));
        let probe = s.read_with(cx, |s, _| s.worker_probe());
        probe.held.store(true, SeqCst);
        tick(cx, 221);
        assert_eq!(probe.starts.load(SeqCst), 1);
        s.update(cx, |s, cx| {
            s.transfer(cx).unwrap();
            for i in 2..100 {
                s.set_draft(format!("{{\"n\":{i}}}"), Mode::Minify, cx);
            }
            assert!(s.running);
            assert!(s.busy);
        });
        tick(cx, 1000);
        assert_eq!(probe.starts.load(SeqCst), 1);
        probe.held.store(false, SeqCst);
        tick(cx, 11);
        tick(cx, 221);
        assert_eq!(probe.starts.load(SeqCst), 2);
        s.update(cx, |s, _| {
            assert_eq!(s.output, "{\"n\":99}");
            assert!(!s.busy);
            assert!(!s.running);
        });
    }
    #[gpui::test]
    fn physical_cancel_and_retire_fence_late_success(cx: &mut TestAppContext) {
        use std::sync::atomic::Ordering::SeqCst;
        let s = cx.new(|cx| JsonSession::new("[]".into(), true, cx));
        let probe = s.read_with(cx, |s, _| s.worker_probe());
        probe.held.store(true, SeqCst);
        tick(cx, 221);
        assert_eq!(probe.starts.load(SeqCst), 1);
        s.update(cx, |s, cx| {
            s.cancel(cx);
            s.retire(cx);
            s.set_draft("{}".into(), Mode::Validate, cx);
        });
        probe.held.store(false, SeqCst);
        tick(cx, 11);
        s.update(cx, |s, _| {
            assert!(s.output.is_empty());
            assert!(!s.busy);
            assert!(s.desired.is_none());
            assert_eq!(s.mode, Mode::Pretty);
        });
        assert_eq!(probe.starts.load(SeqCst), 1);
    }
    #[gpui::test]
    fn oversized_full_draft_never_parses_the_previous_valid_input(cx: &mut TestAppContext) {
        let s = cx.new(|cx| JsonSession::new("{}".into(), false, cx));
        s.update(cx, |s, cx| {
            s.set_input("x".repeat(bellobox_core::MAX_INPUT_BYTES + 1), cx);
            s.set_mode(Mode::Validate, cx);
            s.refresh(cx);
            assert!(s.error.is_some());
            assert!(s.desired.is_none());
        });
        tick(cx, 1000);
        s.update(cx, |s, _| assert!(s.output.is_empty()));
    }
    #[cfg(feature = "developer-tools")]
    #[gpui::test]
    fn cancelled_preview_restarts_only_on_explicit_transfer_after_physical_drain(
        cx: &mut TestAppContext,
    ) {
        use std::sync::atomic::Ordering::SeqCst;
        let s = cx.new(|cx| JsonSession::new("[7]".into(), true, cx));
        let probe = s.read_with(cx, |s, _| s.worker_probe());
        probe.held.store(true, SeqCst);
        tick(cx, 221);
        s.update(cx, |s, cx| {
            s.cancel(cx);
            assert!(!s.busy);
            s.transfer(cx).unwrap();
            assert!(s.busy);
        });
        tick(cx, 1000);
        assert_eq!(probe.starts.load(SeqCst), 1);
        probe.held.store(false, SeqCst);
        tick(cx, 11);
        tick(cx, 221);
        assert_eq!(probe.starts.load(SeqCst), 2);
        s.update(cx, |s, _| {
            assert!(s.can_copy());
            assert!(s.output.contains('7'));
        });
    }
}
