//! Pointer-free bounded one-shot used by the native deactivation subscription.
//! No executor dependency, native object, polling loop or window mutation.
#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
use crate::native_capture::CaptureCancellation;
#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationDeactivationEvent {
    /// The application's deactivation was observed. Its transaction cancellation
    /// flag is already set before this result becomes visible to the waiter.
    ResignedActive,
    /// Subscription removed before deactivation. End this wait only; never treat
    /// this event as an instruction to cancel a successor transaction.
    ObserverRemoved,
}

#[derive(Default)]
struct State {
    result: Option<ApplicationDeactivationEvent>,
    waker: Option<Waker>,
}

/// Single-consumer Future for an App-owned id/generation-checked cleanup task.
/// Drop the UI observer to unregister and resolve a pending wait. Dropping this
/// signal alone does not unregister or cancel the transaction.
pub struct ApplicationDeactivationSignal(Arc<Mutex<State>>);
impl Future for ApplicationDeactivationSignal {
    type Output = ApplicationDeactivationEvent;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(result) = state.result {
            Poll::Ready(result)
        } else {
            if !state
                .waker
                .as_ref()
                .is_some_and(|waker| waker.will_wake(cx.waker()))
            {
                state.waker = Some(cx.waker().clone());
            }
            Poll::Pending
        }
    }
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
#[derive(Clone)]
pub(super) struct Notifier(Arc<Mutex<State>>);
#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
impl Notifier {
    pub(super) fn pair() -> (Self, ApplicationDeactivationSignal) {
        let state = Arc::new(Mutex::new(State::default()));
        (Self(state.clone()), ApplicationDeactivationSignal(state))
    }

    /// The same terminal claim serializes callback versus observer Drop. Once
    /// close wins, a queued native callback cannot cancel the retired transaction.
    pub(super) fn resigned(&self, cancellation: &CaptureCancellation) {
        let wake = {
            let mut state = self
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.result.is_some() {
                return;
            }
            cancellation.cancel();
            state.result = Some(ApplicationDeactivationEvent::ResignedActive);
            state.waker.take()
        };
        wake_safely(wake);
    }

    /// Mark closed BEFORE native removal; return the pending waker so the UI
    /// owner can unregister/release its token before waking the cleanup task.
    pub(super) fn close(&self) -> Option<Waker> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.result.is_some() {
            return None;
        }
        state.result = Some(ApplicationDeactivationEvent::ObserverRemoved);
        state.waker.take()
    }
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
pub(super) fn wake_safely(wake: Option<Waker>) {
    if let Some(wake) = wake {
        // User-provided Wakers can run arbitrary Rust. Never unwind through the
        // Objective-C block or the UI observer's Drop, and never wake under lock.
        let _ = catch_unwind(AssertUnwindSafe(|| wake.wake()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        task::Wake,
    };

    struct CountWake(AtomicUsize);
    impl Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn poll(
        signal: &mut ApplicationDeactivationSignal,
        count: &Arc<CountWake>,
    ) -> Poll<ApplicationDeactivationEvent> {
        let waker = Waker::from(count.clone());
        Pin::new(signal).poll(&mut Context::from_waker(&waker))
    }
    fn counter() -> Arc<CountWake> {
        Arc::new(CountWake(AtomicUsize::new(0)))
    }

    #[test]
    fn notification_cancels_before_publication_and_duplicate_or_drop_cannot_replace_it() {
        let (notify, mut signal) = Notifier::pair();
        let cancellation = CaptureCancellation::default();
        let wake = counter();
        assert!(poll(&mut signal, &wake).is_pending());
        notify.resigned(&cancellation);
        assert!(cancellation.is_cancelled());
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert_eq!(
            poll(&mut signal, &wake),
            Poll::Ready(ApplicationDeactivationEvent::ResignedActive)
        );
        notify.resigned(&cancellation);
        wake_safely(notify.close());
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert_eq!(
            poll(&mut signal, &wake),
            Poll::Ready(ApplicationDeactivationEvent::ResignedActive)
        );
    }

    #[test]
    fn close_wins_before_late_callback_without_cancelling() {
        let (notify, mut signal) = Notifier::pair();
        let late = notify.clone();
        let cancellation = CaptureCancellation::default();
        let wake = counter();
        assert!(poll(&mut signal, &wake).is_pending());
        let deferred_wake = notify.close();
        late.resigned(&cancellation);
        assert!(!cancellation.is_cancelled());
        assert_eq!(
            poll(&mut signal, &wake),
            Poll::Ready(ApplicationDeactivationEvent::ObserverRemoved)
        );
        assert_eq!(wake.0.load(Ordering::SeqCst), 0);
        wake_safely(deferred_wake);
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn event_before_poll_is_retained_and_latest_registered_waker_is_used() {
        let (notify, mut signal) = Notifier::pair();
        notify.resigned(&CaptureCancellation::default());
        let first = counter();
        assert_eq!(
            poll(&mut signal, &first),
            Poll::Ready(ApplicationDeactivationEvent::ResignedActive)
        );
        let (notify, mut signal) = Notifier::pair();
        let second = counter();
        assert!(poll(&mut signal, &first).is_pending());
        assert!(poll(&mut signal, &second).is_pending());
        notify.resigned(&CaptureCancellation::default());
        assert_eq!(first.0.load(Ordering::SeqCst), 0);
        assert_eq!(second.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn retired_subscription_cannot_cancel_an_independent_successor() {
        let (old, _) = Notifier::pair();
        let (new, mut signal) = Notifier::pair();
        let old_cancel = CaptureCancellation::default();
        let new_cancel = CaptureCancellation::default();
        wake_safely(old.close());
        old.resigned(&old_cancel);
        assert!(!old_cancel.is_cancelled() && !new_cancel.is_cancelled());
        new.resigned(&new_cancel);
        assert!(new_cancel.is_cancelled());
        assert_eq!(
            poll(&mut signal, &counter()),
            Poll::Ready(ApplicationDeactivationEvent::ResignedActive)
        );
    }

    #[test]
    fn signal_is_send_sync_and_waker_panic_cannot_escape_notification() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ApplicationDeactivationSignal>();
        struct PanicWake;
        impl Wake for PanicWake {
            fn wake(self: Arc<Self>) {
                panic!("test waker panic");
            }
        }
        let (notify, mut signal) = Notifier::pair();
        let waker = Waker::from(Arc::new(PanicWake));
        assert!(Pin::new(&mut signal)
            .poll(&mut Context::from_waker(&waker))
            .is_pending());
        let cancellation = CaptureCancellation::default();
        notify.resigned(&cancellation);
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn concurrent_notification_and_close_have_one_consistent_terminal_result() {
        for _ in 0..32 {
            let (notify, mut signal) = Notifier::pair();
            let cancellation = CaptureCancellation::default();
            let worker_cancel = cancellation.clone();
            let worker_notify = notify.clone();
            let worker = std::thread::spawn(move || worker_notify.resigned(&worker_cancel));
            wake_safely(notify.close());
            worker.join().unwrap();
            match poll(&mut signal, &counter()) {
                Poll::Ready(ApplicationDeactivationEvent::ResignedActive) => {
                    assert!(cancellation.is_cancelled())
                }
                Poll::Ready(ApplicationDeactivationEvent::ObserverRemoved) => {
                    assert!(!cancellation.is_cancelled())
                }
                Poll::Pending => panic!("terminal signal was lost"),
            }
        }
    }
}
