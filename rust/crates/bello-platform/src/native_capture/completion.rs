//! Native callbacks hold Arc<Job<T>>, never pointers into a waiting stack.
//! A terminal result, cancellation or deadline wins once; later payloads drop.
use super::*;
use std::sync::{Condvar, Mutex};
use std::time::Instant;

/// Every native callback and queued/running encoder holds the same Arc lease.
/// Timeout/cancellation only abandons the Job, never this independent ownership.
pub(super) struct InflightGuard {
    slot: &'static AtomicBool,
    drained: Arc<(Mutex<bool>, Condvar)>,
}
impl InflightGuard {
    pub(super) fn acquire(slot: &'static AtomicBool) -> CaptureResult<Arc<Self>> {
        slot.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| CaptureError::Busy)?;
        Ok(Arc::new(Self {
            slot,
            drained: Arc::new((Mutex::new(false), Condvar::new())),
        }))
    }
}
/// Separate waiter ownership never retains the admission lease itself.
#[cfg(any(test, target_os = "macos"))]
pub(super) struct PhysicalDrain {
    state: Arc<(Mutex<bool>, Condvar)>,
    #[cfg(test)]
    result_notice: Option<std::sync::mpsc::Sender<bool>>,
}
#[cfg(any(test, target_os = "macos"))]
impl PhysicalDrain {
    #[cfg(test)]
    pub(super) fn with_result_notice(mut self, notice: std::sync::mpsc::Sender<bool>) -> Self {
        self.result_notice = Some(notice);
        self
    }
    #[cfg(test)]
    fn note_wait_result(&self, succeeded: bool) {
        if let Some(notice) = &self.result_notice {
            let _ = notice.send(succeeded);
        }
    }
    pub(super) fn wait(self) {
        let (lock, changed) = &*self.state;
        let mut drained = lock.lock().unwrap_or_else(|p| p.into_inner());
        while !*drained {
            drained = changed.wait(drained).unwrap_or_else(|p| p.into_inner());
        }
    }
}
#[cfg(any(test, target_os = "macos"))]
impl InflightGuard {
    pub(super) fn drain(&self) -> PhysicalDrain {
        PhysicalDrain {
            state: self.drained.clone(),
            #[cfg(test)]
            result_notice: None,
        }
    }
}
impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.slot.store(false, Ordering::Release);
        let (lock, changed) = &*self.drained;
        *lock.lock().unwrap_or_else(|p| p.into_inner()) = true;
        changed.notify_all();
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
    #[cfg(any(test, target_arch = "aarch64"))]
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
        Self::with_deadline(cancellation, Instant::now() + timeout)
    }
    pub(super) fn with_deadline(cancellation: CaptureCancellation, deadline: Instant) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                stage: Stage::InitialContent,
                result: None,
            }),
            changed: Condvar::new(),
            cancellation,
            deadline,
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
    /// The result is only logically complete until all native owners release.
    /// Recheck after physical drain, dropping expired success on this worker.
    pub(super) fn wait_drained(&self, drain: PhysicalDrain) -> Result<T, E> {
        let result = self.wait();
        #[cfg(test)]
        drain.note_wait_result(result.is_ok());
        drain.wait();
        if let Some(error) = self.interruption() {
            return Err(error.into());
        }
        result
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
