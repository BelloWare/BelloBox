use super::*;
use gpui::AppContext;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[test]
fn request_cancels_once_but_waits_for_each_physical_ticket() {
    let registry = Registry::default();
    let cancelled = Arc::new(AtomicUsize::new(0));
    let tickets = (0..3)
        .map(|_| {
            let cancelled = cancelled.clone();
            registry
                .admit(move || {
                    cancelled.fetch_add(1, Ordering::AcqRel);
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(registry.request());
    assert!(!registry.request());
    assert_eq!(cancelled.load(Ordering::Acquire), 3);
    let late = Arc::new(AtomicBool::new(false));
    let cancelled_late = late.clone();
    assert!(
        registry
            .admit(move || cancelled_late.store(true, Ordering::Release))
            .is_none()
    );
    assert!(late.load(Ordering::Acquire));
    let mut drain = Box::pin(registry.clone().drained());
    let mut context = Context::from_waker(Waker::noop());
    assert!(drain.as_mut().poll(&mut context).is_pending());
    for (remaining, ticket) in tickets.into_iter().rev().enumerate() {
        drop(ticket);
        assert_eq!(drain.as_mut().poll(&mut context).is_ready(), remaining == 2);
    }
    assert_eq!(registry.active(), 0);
}

#[test]
fn physical_ticket_drop_wakes_the_registered_quit_waiter() {
    struct Wake(AtomicUsize);
    impl std::task::Wake for Wake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }
    }
    let registry = Registry::default();
    let ticket = registry.admit(|| {}).unwrap();
    registry.request();
    let wake = Arc::new(Wake(AtomicUsize::new(0)));
    let waker = Waker::from(wake.clone());
    let mut context = Context::from_waker(&waker);
    let mut drain = Box::pin(registry.drained());
    assert!(drain.as_mut().poll(&mut context).is_pending());
    std::thread::spawn(move || drop(ticket)).join().unwrap();
    assert_eq!(wake.0.load(Ordering::Acquire), 1);
    assert!(drain.as_mut().poll(&mut context).is_ready());
}

struct Retained;
impl gpui::Render for Retained {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
    }
}
#[gpui::test]
fn last_window_close_waits_without_timeout_with_retained_entity_and_held_worker(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let window = cx.add_window(|_, _| Retained);
    let retained = window.root(cx).unwrap();
    let registry = cx.update(registry);
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let ticket = registry
        .admit(move || signal.store(true, Ordering::Release))
        .unwrap();
    let (release, hold) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ticket = ticket;
        hold.recv().unwrap();
    });
    window
        .update(cx, |_, window, cx| close_window(window, cx))
        .unwrap();
    cx.run_until_parked();
    assert!(cancelled.load(Ordering::Acquire));
    assert_eq!(
        cx.read(|cx| cx.windows().len()),
        1,
        "native loop must retain its final window"
    );
    cx.update(|cx| {
        cx.update_window(window.into(), |_, window, _| assert!(is_closing(window)))
            .unwrap()
    });
    assert_eq!(cx.read(quit_calls), 0);
    assert_eq!(registry.active(), 1);
    // Arbitrarily advancing past GPUI's 100 ms budget or native logical timeout
    // must not release physical admission. Only the worker's ticket can do so.
    cx.background_executor
        .advance_clock(std::time::Duration::from_secs(3600));
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 0);
    cx.update(request_quit);
    assert_eq!(registry.active(), 1);
    release.send(()).unwrap();
    worker.join().unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
    retained.read_with(cx, |_, _| ());
    cx.update(request_quit);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1, "quit latch must be one-shot");
}

#[gpui::test]
fn replacement_window_before_queued_last_close_does_not_request_quit(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let first = cx.add_window(|_, _| Retained);
    first
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    let _replacement = cx.add_window(|_, _| Retained);
    cx.run_until_parked();
    assert!(!cx.read(requested));
    assert_eq!(cx.read(quit_calls), 0);
}

#[gpui::test]
fn latched_quit_rejects_new_tool_and_launcher_windows(cx: &mut gpui::TestAppContext) {
    cx.update(init);
    let registry = cx.update(registry);
    let ticket = registry.admit(|| {}).unwrap();
    cx.update(request_quit);
    cx.update(|cx| {
        crate::desktop::open_tool("recording", String::new(), cx);
        crate::desktop::open_tool("videoToGIF", String::new(), cx);
        crate::desktop::open_tool("settings", String::new(), cx);
        crate::desktop::open_launcher(String::new(), cx);
        assert!(cx.windows().is_empty());
    });
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 0);
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn dropped_completion_retains_ticket_until_background_disposes_terminal_payload(
    cx: &mut gpui::TestAppContext,
) {
    struct Probe(Arc<AtomicBool>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let disposed = Arc::new(AtomicBool::new(false));
    let probe = Probe(disposed.clone());
    let completion = cx.update(|cx| admit(cx, || {}).unwrap().spawn(cx, move || probe));
    let registry = cx.update(registry);
    cx.run_until_parked();
    assert!(!disposed.load(Ordering::Acquire));
    assert_eq!(
        registry.active(),
        1,
        "sending a result is not receiver transfer"
    );
    drop(completion);
    assert!(
        !disposed.load(Ordering::Acquire),
        "foreground drop only closes receiver"
    );
    assert_eq!(registry.active(), 1, "physical disposer has not run");
    cx.update(request_quit);
    cx.run_until_parked();
    assert!(disposed.load(Ordering::Acquire));
    assert_eq!(registry.active(), 0);
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn accepted_completion_handoff_retires_worker_without_destroying_adopted_payload(
    cx: &mut gpui::TestAppContext,
) {
    let completion = cx.update(|cx| {
        admit(cx, || {})
            .unwrap()
            .spawn(cx, || String::from("accepted"))
    });
    let registry = cx.update(registry);
    cx.run_until_parked();
    assert_eq!(registry.active(), 1);
    let mut completion = completion;
    let mut context = Context::from_waker(Waker::noop());
    assert!(
        Box::pin(completion.ready())
            .as_mut()
            .poll(&mut context)
            .is_ready()
    );
    assert_eq!(registry.active(), 1, "readiness is not ownership transfer");
    let result = completion.take().unwrap();
    assert_eq!(
        registry.active(),
        1,
        "transfer wakes worker; receiver cannot release admission"
    );
    cx.run_until_parked();
    assert_eq!(registry.active(), 0);
    assert_eq!(result, "accepted");
}

#[gpui::test]
fn rejected_admission_does_not_construct_or_receive_an_owning_work_closure(
    cx: &mut gpui::TestAppContext,
) {
    let built = Arc::new(AtomicBool::new(false));
    cx.update(request_quit);
    cx.update(|cx| {
        if let Some(permit) = admit::<String>(cx, || {}) {
            built.store(true, Ordering::Release);
            let _completion = permit.spawn(cx, || String::from("must not run"));
        }
    });
    assert!(!built.load(Ordering::Acquire));
    cx.run_until_parked();
    assert_eq!(cx.update(registry).active(), 0);
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn unwinding_worker_wakes_completion_and_releases_ticket_after_owned_capture_cleanup(
    cx: &mut gpui::TestAppContext,
) {
    let permit = cx.update(|cx| admit::<String>(cx, || {}).unwrap());
    let registry = cx.update(registry);
    let completion = Completion(permit.shared.clone());
    struct Capture {
        dropped: Arc<AtomicBool>,
        registry: Registry,
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            assert_eq!(
                self.registry.active(),
                1,
                "capture cleanup precedes ticket drop"
            );
            self.dropped.store(true, Ordering::Release);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let capture = Capture {
        dropped: dropped.clone(),
        registry: registry.clone(),
    };
    let worker = std::thread::spawn(move || {
        let _ticket = permit.ticket;
        let _end = WorkerEnd(permit.shared);
        let _capture = capture;
        panic!("injected work panic");
    });
    assert!(worker.join().is_err());
    assert!(dropped.load(Ordering::Acquire));
    assert_eq!(registry.active(), 0);
    let mut context = Context::from_waker(Waker::noop());
    assert!(
        Box::pin(completion.ready())
            .as_mut()
            .poll(&mut context)
            .is_ready()
    );
    let mut completion = completion;
    assert!(completion.take().is_none());
}

#[gpui::test]
fn a_different_last_window_retains_native_lifetime_for_already_retiring_media(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let media = cx.add_window(|_, _| Retained);
    let home = cx.add_window(|window, cx| {
        guard_window(window, cx);
        Retained
    });
    let ticket = cx.update(|cx| registry(cx).admit(|| {}).unwrap());
    media
        .update(cx, |_, window, cx| close_window(window, cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);
    assert!(!cx.read(requested));
    home.update(cx, |_, window, cx| assert!(!allow_close(window, cx)))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.read(requested));
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);
    assert_eq!(cx.read(quit_calls), 0);
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
}
