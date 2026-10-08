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
    cx.update(|cx| cx.dispatch_action(&Quit));
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

#[gpui::test]
fn quit_action_without_an_active_window_waits_for_every_physical_worker(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    assert!(cx.read(|cx| cx.active_window().is_none()));
    let registry = cx.update(registry);
    let cancellations = Arc::new(AtomicUsize::new(0));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let signal = cancellations.clone();
        let ticket = registry
            .admit(move || {
                signal.fetch_add(1, Ordering::AcqRel);
            })
            .unwrap();
        let (release, hold) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ticket = ticket;
            hold.recv().unwrap();
        });
        workers.push((release, worker));
    }
    cx.update(|cx| cx.dispatch_action(&Quit));
    cx.run_until_parked();
    assert!(cx.read(requested));
    assert_eq!(cancellations.load(Ordering::Acquire), 2);
    assert!(cx.read(|cx| cx.windows().is_empty()));
    cx.background_executor
        .advance_clock(std::time::Duration::from_secs(3600));
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 0);
    for (index, (release, worker)) in workers.into_iter().enumerate() {
        cx.update(|cx| cx.dispatch_action(&Quit));
        release.send(()).unwrap();
        worker.join().unwrap();
        cx.run_until_parked();
        assert_eq!(registry.active(), 1 - index);
        assert_eq!(cx.read(quit_calls), index);
    }
    cx.update(|cx| cx.dispatch_action(&Quit));
    cx.run_until_parked();
    assert_eq!(cancellations.load(Ordering::Acquire), 2);
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn window_quit_action_defers_all_roots_and_vetoes_repeated_mixed_closes(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let first = cx.add_window(|window, cx| {
        guard_window(window, cx);
        Retained
    });
    let second = cx.add_window(|window, cx| {
        guard_window(window, cx);
        Retained
    });
    let retained = first.root(cx).unwrap();
    let registry = cx.update(registry);
    let cancellations = Arc::new(AtomicUsize::new(0));
    let signal = cancellations.clone();
    let ticket = registry
        .admit(move || {
            signal.fetch_add(1, Ordering::AcqRel);
        })
        .unwrap();
    // Actual GPUI window dispatch invokes the global listener while borrowing
    // this window. Synchronous root updates would fail or skip the active root.
    cx.dispatch_action(first.into(), Quit);
    for handle in [first.into(), second.into()] {
        cx.update_window(handle, |_, window, _| assert!(is_closing(window)))
            .unwrap();
    }
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);
    let mut visual = gpui::VisualTestContext::from_window(first.into(), cx);
    assert!(!visual.simulate_close());
    cx.dispatch_action(second.into(), Quit);
    cx.update_window(second.into(), |_, window, cx| close_window(window, cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);
    assert_eq!(cx.read(quit_calls), 0);
    assert_eq!(cancellations.load(Ordering::Acquire), 1);
    cx.background_executor
        .advance_clock(std::time::Duration::from_secs(3600));
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 0);
    retained.read_with(cx, |_, _| ());
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn app_quit_with_inactive_existing_window_still_shows_closing(cx: &mut gpui::TestAppContext) {
    cx.update(init);
    let window = cx.add_window(|window, cx| {
        guard_window(window, cx);
        Retained
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.deactivate_window();
    assert!(cx.read(|cx| cx.active_window().is_none()));
    let ticket = cx.update(|cx| registry(cx).admit(|| {}).unwrap());
    cx.update(|cx| cx.dispatch_action(&Quit));
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, _| assert!(is_closing(window)))
        .unwrap();
    assert_eq!(cx.read(quit_calls), 0);
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn quit_shortcut_bubbles_from_focused_editor_without_replacing_edit_shortcuts(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let window = cx.add_window(|window, cx| {
        guard_window(window, cx);
        let editor = bello_workbench_ui::EditorView::new("kept text".into(), window, cx);
        editor.focus(window);
        editor
    });
    let editor = window.root(cx).unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    let select_copy = if cfg!(target_os = "macos") {
        "cmd-a cmd-c"
    } else {
        "ctrl-a ctrl-c"
    };
    visual.simulate_keystrokes(select_copy);
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("kept text".into())
    );
    visual.simulate_keystrokes("q");
    assert!(!cx.read(requested), "plain q remains text input");
    editor.read_with(cx, |editor, _| assert_eq!(editor.text(), "q"));
    let ticket = cx.update(|cx| registry(cx).admit(|| {}).unwrap());
    visual.simulate_keystrokes(QUIT_KEYSTROKE);
    assert!(cx.read(requested));
    visual.update(|window, _| assert!(is_closing(window)));
    assert_eq!(cx.read(quit_calls), 0);
    // The replacement root has no editor focus handle. Repeat still resolves
    // the global binding and must not bypass the existing physical drain.
    visual.simulate_keystrokes(QUIT_KEYSTROKE);
    assert_eq!(cx.read(quit_calls), 0);
    editor.read_with(cx, |editor, _| assert_eq!(editor.text(), "q"));
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn repeated_quit_action_without_work_calls_final_quit_once(cx: &mut gpui::TestAppContext) {
    cx.update(init);
    cx.update(|cx| {
        cx.dispatch_action(&Quit);
        cx.dispatch_action(&Quit);
    });
    cx.run_until_parked();
    assert!(cx.read(requested));
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn refused_quit_keeps_media_and_roots_live_then_rechecks_without_saved_approval(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let blocked = Arc::new(AtomicBool::new(true));
    let flag = blocked.clone();
    let window = cx.add_window(|window, cx| {
        guard_window(window, cx);
        guard_quit(window, cx, move |_, _, _| {
            if flag.load(Ordering::Acquire) {
                QuitAdmission::Explain("Finish editing before quitting.")
            } else {
                QuitAdmission::Ready
            }
        });
        Retained
    });
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let ticket = cx.update(|cx| {
        registry(cx)
            .admit(move || signal.store(true, Ordering::Release))
            .unwrap()
    });
    cx.dispatch_action(window.into(), Quit);
    assert!(!cx.read(requested));
    assert!(!cancelled.load(Ordering::Acquire));
    assert!(window.root(cx).is_ok());
    let prompt = crate::shutdown::pending_refusal(cx);
    assert!(prompt.is_some());
    for _ in 0..3 {
        cx.dispatch_action(window.into(), Quit);
        assert_eq!(crate::shutdown::pending_refusal(cx), prompt);
    }
    crate::shutdown::dismiss_refusal(cx);
    cx.run_until_parked();
    assert!(
        !crate::shutdown::pending_refusal(cx).is_some(),
        "one acknowledgment dismisses one prompt"
    );
    assert!(!cx.read(requested));
    let fresh = cx.update(|cx| registry(cx).admit(|| {}).unwrap());
    drop(fresh);
    blocked.store(false, Ordering::Release);
    assert!(!cx.read(requested), "refusal has no queued continuation");
    cx.dispatch_action(window.into(), Quit);
    assert!(cx.read(requested));
    assert!(cancelled.load(Ordering::Acquire));
    assert_eq!(cx.read(quit_calls), 0);
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn closed_guarded_owner_cannot_veto_and_registry_does_not_retain_it(cx: &mut gpui::TestAppContext) {
    cx.update(init);
    let first = cx.add_window(|window, cx| {
        guard_window(window, cx);
        guard_quit(window, cx, |_, _, _| {
            QuitAdmission::Explain("Still editing.")
        });
        Retained
    });
    let weak = first.root(cx).unwrap().downgrade();
    let second = cx.add_window(|window, cx| {
        guard_window(window, cx);
        Retained
    });
    first
        .update(cx, |_, window, cx| close_window(window, cx))
        .unwrap();
    cx.run_until_parked();
    assert!(weak.upgrade().is_none());
    cx.dispatch_action(second.into(), Quit);
    assert!(cx.read(requested));
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn unregistered_root_fails_closed_with_deduplicated_feedback(cx: &mut gpui::TestAppContext) {
    cx.update(init);
    let window = cx.add_window(|_, _| Retained);
    cx.dispatch_action(window.into(), Quit);
    assert!(!cx.read(requested));
    assert!(
        crate::shutdown::pending_refusal(cx)
            .unwrap()
            .1
            .contains("Close the capture or tool window")
    );
    cx.dispatch_action(window.into(), Quit);
    crate::shutdown::dismiss_refusal(cx);
    cx.run_until_parked();
    assert!(!crate::shutdown::pending_refusal(cx).is_some());
    assert!(window.root(cx).is_ok());
}

#[gpui::test]
fn physical_nonmedia_guard_survives_host_close_until_every_owner_retires(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let owner = cx.add_window(|window, cx| {
        guard_window(window, cx);
        Retained
    });
    let home = cx.add_window(|window, cx| {
        guard_window(window, cx);
        Retained
    });
    let blocker = cx.update(|cx| block_quit(cx, "A QR image is still being saved."));
    let worker_blocker = blocker.clone();
    let (release, hold) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _physical_owner = worker_blocker;
        hold.recv().unwrap();
    });
    owner
        .update(cx, |_, window, cx| close_window(window, cx))
        .unwrap();
    drop(blocker);
    cx.dispatch_action(home.into(), Quit);
    assert!(!cx.read(requested));
    assert!(home.root(cx).is_ok());
    cx.background_executor
        .advance_clock(std::time::Duration::from_secs(3600));
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 0);
    release.send(()).unwrap();
    worker.join().unwrap();
    crate::shutdown::dismiss_refusal(cx);
    cx.run_until_parked();
    assert!(
        !cx.read(requested),
        "physical completion never resumes a refused Quit"
    );
    cx.dispatch_action(home.into(), Quit);
    assert_eq!(cx.read(quit_calls), 1);
}

#[gpui::test]
fn actual_home_settings_and_media_roots_are_admitted_before_one_final_quit(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(init);
    let home = cx.add_window(|window, cx| {
        guard_window(window, cx);
        crate::home::Home::new(window, cx)
    });
    cx.update(|cx| {
        crate::settings_ui::open(cx);
        crate::recording_ui::open(cx);
        crate::gif_converter::open(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| cx.windows().len()), 4);
    let ticket = cx.update(|cx| registry(cx).admit(|| {}).unwrap());
    cx.dispatch_action(home.into(), Quit);
    assert!(cx.read(requested));
    assert!(!crate::shutdown::pending_refusal(cx).is_some());
    assert_eq!(cx.read(|cx| cx.windows().len()), 4);
    assert_eq!(cx.read(quit_calls), 0);
    cx.update(|cx| {
        for handle in cx.windows() {
            cx.update_window(handle, |_, window, _| assert!(is_closing(window)))
                .unwrap();
        }
    });
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(quit_calls), 1);
}
