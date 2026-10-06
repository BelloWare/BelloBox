//! Native callbacks hold Arc<Job<T>>, never pointers into a waiting stack.
//! A terminal result, cancellation or deadline wins once; later payloads drop.
use super::*;
use std::sync::{Condvar, Mutex};
use std::time::Instant;

/// Every native callback and queued/running encoder holds the same Arc lease.
/// Timeout/cancellation only abandons the Job, never this independent ownership.
pub(super) struct InflightGuard {
    slot: &'static AtomicBool,
}
impl InflightGuard {
    pub(super) fn acquire(slot: &'static AtomicBool) -> CaptureResult<Arc<Self>> {
        slot.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| CaptureError::Busy)?;
        Ok(Arc::new(Self { slot }))
    }
}
impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.slot.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    InitialContent,
    ResolvingInitial,
    RefreshedContent,
    ResolvingRefreshed,
    Image,
    EncodingQueued,
    Encoding,
    #[cfg(test)]
    Validating,
    Completed,
    Abandoned,
}
struct State<T, E> {
    stage: Stage,
    result: Option<Result<T, E>>,
}
pub(super) struct Job<T, E = CaptureError> {
    state: Mutex<State<T, E>>,
    changed: Condvar,
    cancellation: CaptureCancellation,
    deadline: Instant,
}
impl<T, E: From<CaptureError>> Job<T, E> {
    pub(super) fn new(cancellation: CaptureCancellation, timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                stage: Stage::InitialContent,
                result: None,
            }),
            changed: Condvar::new(),
            cancellation,
            deadline: Instant::now() + timeout,
        })
    }
    fn interruption(&self) -> Option<CaptureError> {
        if self.cancellation.is_cancelled() {
            Some(CaptureError::Cancelled)
        } else if Instant::now() >= self.deadline {
            Some(CaptureError::TimedOut)
        } else {
            None
        }
    }
    pub(super) fn active(&self) -> bool {
        if self.interruption().is_some() {
            return false;
        }
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        !matches!(state.stage, Stage::Completed | Stage::Abandoned)
    }
    /// Only one callback can claim a phase, even if a native callback misfires twice.
    pub(super) fn transition(&self, from: Stage, to: Stage) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if self.interruption().is_some() || state.stage != from {
            return false;
        }
        state.stage = to;
        true
    }
    pub(super) fn complete(&self, value: Result<T, E>) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if self.interruption().is_some()
            || matches!(state.stage, Stage::Completed | Stage::Abandoned)
        {
            return false;
        }
        state.stage = Stage::Completed;
        state.result = Some(value);
        self.changed.notify_all();
        true
    }
    pub(super) fn wait(&self) -> Result<T, E> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(error) = self.interruption() {
                state.stage = Stage::Abandoned;
                state.result = None;
                return Err(error.into());
            }
            if let Some(result) = state.result.take() {
                return result;
            }
            let wait = self
                .deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(5));
            state = self
                .changed
                .wait_timeout(state, wait)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }
}
