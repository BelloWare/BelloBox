use super::*;
use std::{
    collections::VecDeque,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};
static ACTIVE: AtomicBool = AtomicBool::new(false);
const OPEN: u8 = 0;
const CANCELLED: u8 = 1;
const CLAIMED: u8 = 2;
const PUBLISHED: u8 = 3;
/// Cancellation acceptance is not drain; a publication claim is not success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelOutcome {
    Cancelled,
    PublicationClaimed,
}
#[derive(Default)]
pub(super) struct Publication(AtomicU8);
impl Publication {
    pub fn cancel(&self) -> CancelOutcome {
        match self
            .0
            .compare_exchange(OPEN, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(CANCELLED) => CancelOutcome::Cancelled,
            Err(_) => CancelOutcome::PublicationClaimed,
        }
    }
    pub fn check(&self) -> RecordingResult<()> {
        if self.0.load(Ordering::Acquire) == CANCELLED {
            Err(RecordingError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn claim(&self) -> RecordingResult<()> {
        self.0
            .compare_exchange(OPEN, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|phase| {
                if phase == CANCELLED {
                    RecordingError::Cancelled
                } else {
                    RecordingError::Busy
                }
            })
    }
    pub fn published(&self) {
        self.0.store(PUBLISHED, Ordering::Release);
    }
}
pub(super) struct Shared {
    publication: Publication,
    stop: AtomicBool,
    owner_alive: AtomicBool,
    terminal_queued: AtomicBool,
    terminal_taken: AtomicBool,
    events: Mutex<VecDeque<RecordingEvent>>,
    wake: Condvar,
    drained: AtomicBool,
    retirement_owners: Mutex<Vec<Box<dyn Send>>>,
    #[cfg(all(test, target_os = "macos"))]
    fixture_frames_ready: AtomicBool,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            publication: Publication::default(),
            stop: AtomicBool::new(false),
            owner_alive: AtomicBool::new(true),
            terminal_queued: AtomicBool::new(false),
            terminal_taken: AtomicBool::new(false),
            events: Mutex::new(VecDeque::with_capacity(2)),
            wake: Condvar::new(),
            drained: AtomicBool::new(false),
            retirement_owners: Mutex::new(Vec::new()),
            #[cfg(all(test, target_os = "macos"))]
            fixture_frames_ready: AtomicBool::new(false),
        }
    }
}
#[derive(Clone, Default)]
pub struct RecordingControl(pub(super) Arc<Shared>);
impl RecordingControl {
    pub fn stop(&self) {
        self.0.stop.store(true, Ordering::Release);
        self.0.wake.notify_all();
    }
    pub fn cancel(&self) -> CancelOutcome {
        let outcome = self.0.publication.cancel();
        self.stop();
        outcome
    }
    /// Permanently close the event consumer and signal cancellation. This is
    /// nonblocking even when native publication or callbacks are still active.
    pub fn close(&self) {
        self.owner_dropped();
    }
    pub(super) fn retain_until_drained(&self, owner: impl Send + 'static) {
        let mut owners = self
            .0
            .retirement_owners
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if self.is_drained() {
            drop(owners);
            drop(owner);
        } else {
            owners.push(Box::new(owner));
        }
    }
    pub fn is_drained(&self) -> bool {
        self.0.drained.load(Ordering::Acquire)
    }
    #[cfg(all(test, target_os = "macos"))]
    pub(super) fn mark_fixture_frames_ready(&self) {
        self.0.fixture_frames_ready.store(true, Ordering::Release);
    }
    #[cfg(all(test, target_os = "macos"))]
    pub(super) fn fixture_frames_ready(&self) -> bool {
        self.0.fixture_frames_ready.load(Ordering::Acquire)
    }
    pub(super) fn check(&self) -> RecordingResult<()> {
        self.0.publication.check()
    }
    pub(super) fn claim_publication(&self) -> RecordingResult<()> {
        self.0.publication.claim()
    }
    pub(super) fn published(&self) {
        self.0.publication.published();
    }
    pub(super) fn stopped(&self) -> bool {
        self.0.stop.load(Ordering::Acquire)
    }
    pub(super) fn owner_dropped(&self) {
        self.0.owner_alive.store(false, Ordering::Release);
        self.cancel();
    }
    pub(super) fn wait(&self, duration: Duration) {
        let events = self.0.events.lock().unwrap_or_else(|e| e.into_inner());
        if !self.stopped() {
            drop(
                self.0
                    .wake
                    .wait_timeout(events, duration)
                    .unwrap_or_else(|e| e.into_inner()),
            );
        }
    }
    pub(super) fn started(&self) {
        let mut events = self.0.events.lock().unwrap_or_else(|e| e.into_inner());
        if self.0.owner_alive.load(Ordering::Acquire)
            && !self.0.terminal_queued.load(Ordering::Acquire)
            && events.is_empty()
        {
            events.push_back(RecordingEvent::Started);
        }
    }
    pub(super) fn terminal(&self, event: RecordingEvent) -> bool {
        if self.0.terminal_queued.swap(true, Ordering::AcqRel) {
            return false;
        }
        let mut events = self.0.events.lock().unwrap_or_else(|e| e.into_inner());
        // Exactly one Started and one terminal; only the worker publishes.
        debug_assert!(events.len() <= 1);
        events.push_back(event);
        self.0.wake.notify_all();
        true
    }
    pub(super) fn try_event(&self) -> Option<RecordingEvent> {
        let mut events = self.0.events.try_lock().ok()?;
        let event = events.pop_front()?;
        if !matches!(event, RecordingEvent::Started) {
            self.0.terminal_taken.store(true, Ordering::Release);
            self.0.wake.notify_all();
        }
        Some(event)
    }
    /// Worker retains event storage/admission until transfer or owner drop.
    /// UI Drop only changes atomics; rejected payloads retire outside the mutex.
    pub(super) fn retire_terminal(&self) {
        loop {
            let mut events = self.0.events.lock().unwrap_or_else(|e| e.into_inner());
            if !self.0.owner_alive.load(Ordering::Acquire) {
                let retired = std::mem::take(&mut *events);
                drop(events);
                drop(retired);
                return;
            }
            if self.0.terminal_taken.load(Ordering::Acquire) {
                return;
            }
            drop(
                self.0
                    .wake
                    .wait_timeout(events, Duration::from_millis(10))
                    .unwrap_or_else(|e| e.into_inner()),
            );
        }
    }
    #[cfg(test)]
    pub(super) fn hold_events_for_test(
        &self,
    ) -> std::sync::MutexGuard<'_, VecDeque<RecordingEvent>> {
        self.0.events.lock().unwrap_or_else(|e| e.into_inner())
    }
}
/// Release only after native pools/callbacks and rejected event owners retire.
pub(super) struct Admission {
    control: RecordingControl,
}
impl Admission {
    pub fn acquire(control: RecordingControl) -> RecordingResult<Self> {
        ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| RecordingError::Busy)?;
        Ok(Self { control })
    }
}
impl Drop for Admission {
    fn drop(&mut self) {
        let owners = {
            let mut owners = self
                .control
                .0
                .retirement_owners
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            ACTIVE.store(false, Ordering::Release);
            self.control.0.drained.store(true, Ordering::Release);
            std::mem::take(&mut *owners)
        };
        // Destructors never execute under the registration lock. Only physical
        // Admission retirement releases these owners, after callbacks/pools and
        // rejected terminal payloads have been disposed by the worker.
        drop(owners);
    }
}

/// A copied block keeps its ticket until the framework destroys the final copy.
#[derive(Clone, Default)]
pub(super) struct Callbacks(Arc<(Mutex<usize>, Condvar)>);
pub(super) struct CallbackTicket(Callbacks);
impl Callbacks {
    pub fn ticket(&self) -> CallbackTicket {
        *self.0.0.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        CallbackTicket(self.clone())
    }
    pub fn wait(&self) {
        let (count, wake) = &*self.0;
        let mut count = count.lock().unwrap_or_else(|e| e.into_inner());
        while *count != 0 {
            count = wake.wait(count).unwrap_or_else(|e| e.into_inner());
        }
    }
}
impl Drop for CallbackTicket {
    fn drop(&mut self) {
        *self.0.0.0.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        self.0.0.1.notify_all();
    }
}
