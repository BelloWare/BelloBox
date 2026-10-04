//! Window-owned generation guards. A token from another window is never accepted.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobToken {
    session: u64,
    generation: u64,
}
pub struct SessionJobs {
    session: u64,
    generation: u64,
    cancellation: Arc<AtomicBool>,
}
impl Default for SessionJobs {
    fn default() -> Self {
        Self {
            session: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            generation: 0,
            cancellation: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl SessionJobs {
    pub fn begin(&mut self) -> JobToken {
        self.cancel();
        self.cancellation = Arc::new(AtomicBool::new(false));
        JobToken {
            session: self.session,
            generation: self.generation,
        }
    }
    pub fn accepts(&self, token: JobToken) -> bool {
        token.session == self.session
            && token.generation == self.generation
            && !self.cancellation.load(Ordering::Relaxed)
    }
    pub fn cancel(&mut self) {
        self.cancellation.store(true, Ordering::Relaxed);
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancellation.clone()
    }
}
impl Drop for SessionJobs {
    fn drop(&mut self) {
        self.cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_never_share_jobs() {
        let mut a = SessionJobs::default();
        let mut b = SessionJobs::default();
        let first = a.begin();
        let second = b.begin();
        assert!(a.accepts(first));
        assert!(b.accepts(second));
        assert!(!a.accepts(second));
        assert!(!b.accepts(first));
        a.cancel();
        assert!(!a.accepts(first));
        assert!(b.accepts(second));
    }
    #[test]
    fn repeated_requests_reject_late_results() {
        let mut jobs = SessionJobs::default();
        let first = jobs.begin();
        let flag = jobs.cancellation();
        let second = jobs.begin();
        assert!(flag.load(Ordering::Relaxed));
        assert!(!jobs.accepts(first));
        assert!(jobs.accepts(second));
    }
    #[test]
    fn closing_window_signals_cancellation() {
        let flag;
        {
            let mut jobs = SessionJobs::default();
            jobs.begin();
            flag = jobs.cancellation();
            assert!(!flag.load(Ordering::Relaxed));
        }
        assert!(flag.load(Ordering::Relaxed));
    }
}
