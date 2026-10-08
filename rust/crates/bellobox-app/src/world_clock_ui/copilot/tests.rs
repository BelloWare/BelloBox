//! Synthetic GPUI action-dispatch coverage; no live provider or native-pointer claim.
use super::{Action, Settings, WorldClock, clock, protocol};
use bellobox_core::clock::copilot::{Parts, session::Outcome};
use gpui::{Focusable, TestAppContext, WindowHandle};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};

// Worker admission is process-wide, so every Clock HTTP fixture shares this lock.
static TEST_LOCK: Mutex<()> = Mutex::new(());
fn safe_environment() {
    for name in [
        "BELLOBOX_AI_PROVIDER",
        "BELLOBOX_AI_ENDPOINT",
        "BELLOBOX_AI_MODEL",
        "BELLOBOX_AI_KEY",
    ] {
        assert!(
            std::env::var_os(name).is_none(),
            "{name} must be unset for isolated loopback fixtures"
        );
    }
}
fn accept(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match listener.accept() {
            Ok((socket, address)) => {
                assert!(address.ip().is_loopback());
                return socket;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "expected loopback request did not arrive"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("loopback accept failed: {e}"),
        }
    }
}
fn read_request(socket: &mut TcpStream) -> String {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut header_end = None;
    let mut length = 0;
    loop {
        let mut byte = [0];
        assert_eq!(socket.read(&mut byte).unwrap(), 1);
        bytes.push(byte[0]);
        assert!(bytes.len() < 100_000);
        if header_end.is_none() && bytes.ends_with(b"\r\n\r\n") {
            length = String::from_utf8_lossy(&bytes)
                .to_ascii_lowercase()
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            header_end = Some(bytes.len());
        }
        if header_end.is_some_and(|end| bytes.len() == end + length) {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}
fn respond(socket: &mut TcpStream, answer: &str) {
    let delta = serde_json::json!({"choices":[{"delta":{"content":answer}}]});
    let body = format!("data: {delta}\n\ndata: [DONE]\n\n");
    let _ = write!(
        socket,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}
fn settings(path: &std::path::Path, listener: &TcpListener) {
    Settings {
        provider_endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        provider_model: "clock-fixture".into(),
        zone_ids: vec!["UTC".into()],
        anchor_zone_id: "UTC".into(),
        ..Default::default()
    }
    .save(path)
    .unwrap();
}
fn open(path: &std::path::Path, cx: &mut TestAppContext) -> WindowHandle<WorldClock> {
    cx.add_window(|window, cx| {
        WorldClock::new_at("2026-10-08T12:00:00Z".into(), path.to_owned(), window, cx)
    })
}
fn act(view: WindowHandle<WorldClock>, action: Action, cx: &mut TestAppContext) {
    view.update(cx, |view, window, cx| view.act(action, window, cx))
        .unwrap();
}
fn draft(view: WindowHandle<WorldClock>, text: &str, cx: &mut TestAppContext) {
    view.update(cx, |view, _, cx| {
        view.copilot
            .draft
            .update(cx, |editor, cx| editor.set_text(text.into(), cx))
    })
    .unwrap();
}
struct OtherWindow;
impl gpui::Render for OtherWindow {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
    }
}
fn poll(view: &mut WorldClock) {
    if let Some((generation, result)) = view.copilot.worker.poll() {
        view.copilot.session.complete(generation, result);
    }
}
fn finish(view: WindowHandle<WorldClock>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if !view
            .update(cx, |view, _, _| {
                poll(view);
                view.copilot.worker.busy()
            })
            .unwrap()
        {
            break;
        }
        assert!(Instant::now() < deadline, "Clock worker did not retire");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn explicit_send_answer_apply_stale_revision_and_persistence(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let initial = std::fs::read(&path).unwrap();
    let view = open(&path, cx);
    act(view, Action::Copilot, cx);
    draft(view, "Find a time and add Tokyo", cx);
    cx.run_until_parked();
    assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
    assert_eq!(std::fs::read(&path).unwrap(), initial);
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        let request = read_request(&mut socket);
        respond(
            &mut socket,
            r#"{"answer":"Try this slot.","suggestion":{"referenceDate":"2026-10-09T13:00:00Z","timeZoneIDs":["Asia/Tokyo"],"anchorTimeZoneID":"Asia/Tokyo"}}"#,
        );
        request
    });
    act(view, Action::CopilotSend, cx);
    view.update(cx, |view, _, cx| {
        assert!(view.copilot.draft.read(cx).text().is_empty());
        assert_eq!(view.copilot.session.messages().len(), 1);
    })
    .unwrap();
    finish(view, cx);
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v1/chat/completions "));
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["messages"][0]["content"], protocol::SYSTEM_PROMPT);
    assert!(
        body["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("Question: Find a time and add Tokyo")
    );
    assert!(!body.to_string().contains("selected_text"));
    assert_eq!(std::fs::read(&path).unwrap(), initial);
    let (id, revision) = view
        .update(cx, |view, _, _| {
            assert_eq!(
                view.planner.instant,
                clock::parse_instant("2026-10-08T12:00:00Z").unwrap()
            );
            assert_eq!(view.copilot.session.outcome(), &Outcome::Answered);
            let message = view.copilot.session.messages().last().unwrap();
            assert_eq!(message.text, "Try this slot.");
            (message.id, view.planner_revision)
        })
        .unwrap();
    act(view, Action::Day(1), cx);
    let before = view.update(cx, |view, _, _| view.planner.instant).unwrap();
    act(view, Action::CopilotApply(id, revision), cx);
    let current = view
        .update(cx, |view, _, _| {
            assert_eq!(view.planner.instant, before);
            assert!(
                view.copilot
                    .notice
                    .as_ref()
                    .unwrap()
                    .contains("planner changed")
            );
            view.planner_revision
        })
        .unwrap();
    act(view, Action::CopilotApply(id, current), cx);
    view.update(cx, |view, _, _| {
        assert_eq!(
            view.planner.instant,
            clock::parse_instant("2026-10-09T13:00:00Z").unwrap()
        );
        assert_eq!(view.planner.reference.name(), "Asia/Tokyo");
        assert_eq!(
            view.planner
                .zones
                .iter()
                .map(|z| z.name())
                .collect::<Vec<_>>(),
            ["UTC", "Asia/Tokyo"]
        );
        assert!(
            view.copilot
                .session
                .messages()
                .last()
                .unwrap()
                .applied_parts
                .contains(Parts::TIME | Parts::LOCATIONS)
        );
    })
    .unwrap();
    let saved = Settings::load(&path).unwrap();
    assert_eq!(saved.zone_ids, ["UTC", "Asia/Tokyo"]);
    assert_eq!(saved.anchor_zone_id, "Asia/Tokyo");
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("Try this slot")
    );
}

#[gpui::test]
fn invalid_configuration_keeps_draft_and_transcript_empty(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    Settings {
        provider_endpoint: "invalid endpoint".into(),
        ..Default::default()
    }
    .save(&path)
    .unwrap();
    let view = open(&path, cx);
    draft(view, "Keep this question", cx);
    act(view, Action::CopilotSend, cx);
    view.update(cx, |view, _, cx| {
        assert_eq!(view.copilot.draft.read(cx).text(), "Keep this question");
        assert!(view.copilot.session.messages().is_empty());
        assert!(!view.copilot.worker.busy());
        assert!(matches!(view.copilot.session.outcome(), Outcome::Failed(_)));
    })
    .unwrap();
}

#[gpui::test]
fn cancel_retains_physical_lane_retry_has_no_duplicate_and_clear_is_ephemeral(
    cx: &mut TestAppContext,
) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let (admitted, admission) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut first = accept(&listener);
        read_request(&mut first);
        admitted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(8)).unwrap();
        respond(&mut first, "stale answer");
        let mut retry = accept(&listener);
        let request = read_request(&mut retry);
        respond(&mut retry, "retried answer");
        request
    });
    let view = open(&path, cx);
    draft(view, "Remember my question", cx);
    act(view, Action::CopilotSend, cx);
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    act(view, Action::CopilotCancel, cx);
    view.update(cx, |view, _, _| {
        assert_eq!(view.copilot.session.outcome(), &Outcome::Cancelled);
        assert!(view.copilot.worker.physically_active());
    })
    .unwrap();
    act(view, Action::CopilotRetry, cx);
    view.update(cx, |view, _, _| {
        assert!(
            view.copilot
                .notice
                .as_ref()
                .unwrap()
                .contains("still running or stopping")
        )
    })
    .unwrap();
    release.send(()).unwrap();
    finish(view, cx);
    view.update(cx, |view, _, _| {
        assert_eq!(view.copilot.session.messages().len(), 1);
        assert!(view.copilot.session.can_retry());
    })
    .unwrap();
    act(view, Action::CopilotRetry, cx);
    finish(view, cx);
    let request = server.join().unwrap();
    assert!(request.contains("Remember my question"));
    view.update(cx, |view, _, _| {
        assert_eq!(view.copilot.session.messages().len(), 2);
        assert_eq!(view.copilot.session.messages()[1].text, "retried answer");
    })
    .unwrap();
    draft(view, "Unsent draft", cx);
    act(view, Action::CopilotClear, cx);
    view.update(cx, |view, _, cx| {
        assert!(view.copilot.session.messages().is_empty());
        assert_eq!(view.copilot.draft.read(cx).text(), "Unsent draft");
        assert!(!view.copilot.session.can_retry());
    })
    .unwrap();
}

#[gpui::test]
fn close_retained_entity_and_reopen_cannot_bypass_held_request(cx: &mut TestAppContext) {
    // Keep a separate app window alive; this does not claim last-window Quit/drain coverage.
    let _other_window = cx.add_window(|_, _| OtherWindow);
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let (admitted, admission) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        read_request(&mut socket);
        admitted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(8)).unwrap();
        respond(&mut socket, "late closed answer");
    });
    let old = open(&path, cx);
    let retained = old.root(cx).unwrap();
    draft(old, "Old question", cx);
    act(old, Action::CopilotSend, cx);
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    old.update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    retained.update(cx, |view, cx| {
        assert!(view.copilot.closed);
        assert!(view.copilot.session.messages().is_empty());
        assert!(view.copilot.worker.physically_active());
        view.send_copilot(false, None, cx);
        assert!(view.copilot.session.messages().is_empty());
    });
    let fresh = open(&path, cx);
    draft(fresh, "New question", cx);
    act(fresh, Action::CopilotSend, cx);
    fresh
        .update(cx, |view, _, cx| {
            assert!(
                view.copilot
                    .notice
                    .as_ref()
                    .unwrap()
                    .contains("still stopping")
            );
            assert_eq!(view.copilot.draft.read(cx).text(), "New question");
            assert!(view.copilot.session.messages().is_empty());
        })
        .unwrap();
    release.send(()).unwrap();
    server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let busy = retained.update(cx, |view, _| {
            poll(view);
            view.copilot.worker.busy()
        });
        if !busy {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    retained.update(cx, |view, _| {
        assert!(view.copilot.session.messages().is_empty())
    });
    fresh
        .update(cx, |view, _, _| {
            assert!(view.copilot.session.messages().is_empty())
        })
        .unwrap();
}

#[gpui::test]
fn clear_during_held_request_discards_late_reply_and_keeps_draft(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let (admitted, admission) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        read_request(&mut socket);
        admitted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(8)).unwrap();
        respond(&mut socket, "stale after clear");
    });
    let view = open(&path, cx);
    draft(view, "Sent question", cx);
    act(view, Action::CopilotSend, cx);
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    draft(view, "Unsent next question", cx);
    act(view, Action::CopilotClear, cx);
    view.update(cx, |view, _, cx| {
        assert!(view.copilot.worker.physically_active());
        assert!(!view.copilot.session.is_busy());
        assert!(view.copilot.session.messages().is_empty());
        assert!(!view.copilot.session.can_retry());
        assert_eq!(view.copilot.draft.read(cx).text(), "Unsent next question");
    })
    .unwrap();
    act(view, Action::CopilotSend, cx);
    view.update(cx, |view, _, _| {
        assert!(
            view.copilot
                .notice
                .as_ref()
                .unwrap()
                .contains("still running or stopping")
        )
    })
    .unwrap();
    release.send(()).unwrap();
    server.join().unwrap();
    finish(view, cx);
    view.update(cx, |view, _, cx| {
        assert!(view.copilot.session.messages().is_empty());
        assert_eq!(view.copilot.session.outcome(), &Outcome::None);
        assert_eq!(view.copilot.draft.read(cx).text(), "Unsent next question");
    })
    .unwrap();
}

#[gpui::test]
fn explicit_suggested_prompt_sends_without_erasing_unsent_draft(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        let request = read_request(&mut socket);
        respond(&mut socket, "prompt answer");
        request
    });
    let view = open(&path, cx);
    draft(view, "Keep my unsent draft", cx);
    act(
        view,
        Action::CopilotPrompt("Find a meeting time".into()),
        cx,
    );
    finish(view, cx);
    let request = server.join().unwrap();
    assert!(request.contains("Question: Find a meeting time"));
    assert!(!request.contains("Keep my unsent draft"));
    view.update(cx, |view, _, cx| {
        assert_eq!(view.copilot.draft.read(cx).text(), "Keep my unsent draft");
        assert_eq!(view.copilot.session.messages().len(), 2);
        assert_eq!(
            view.copilot.session.messages()[0].text,
            "Find a meeting time"
        );
        assert_eq!(view.copilot.session.messages()[1].text, "prompt answer");
    })
    .unwrap();
}

#[gpui::test]
fn hiding_busy_panel_cancels_and_rejects_late_answer(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let (admitted, admission) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        read_request(&mut socket);
        admitted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(8)).unwrap();
        respond(&mut socket, "late hidden answer");
    });
    let view = open(&path, cx);
    act(view, Action::Copilot, cx);
    draft(view, "Question before hiding", cx);
    act(view, Action::CopilotSend, cx);
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    act(view, Action::Copilot, cx);
    view.update(cx, |view, _, _| {
        assert!(!view.copilot.visible);
        assert!(!view.copilot.session.is_busy());
        assert!(view.copilot.worker.physically_active());
        assert_eq!(view.copilot.session.outcome(), &Outcome::Cancelled);
    })
    .unwrap();
    release.send(()).unwrap();
    server.join().unwrap();
    finish(view, cx);
    act(view, Action::Copilot, cx);
    view.update(cx, |view, _, _| {
        assert!(view.copilot.visible);
        assert_eq!(view.copilot.session.messages().len(), 1);
        assert_eq!(view.copilot.session.outcome(), &Outcome::Cancelled);
    })
    .unwrap();
}

#[gpui::test]
fn local_close_guard_cancels_and_refuses_until_retired_even_with_other_window(
    cx: &mut TestAppContext,
) {
    local_close_guard_flow(true, cx);
}
#[gpui::test]
fn last_window_close_guard_requires_a_fresh_close_after_retirement(cx: &mut TestAppContext) {
    local_close_guard_flow(false, cx);
}
fn local_close_guard_flow(with_other_window: bool, cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let _other = with_other_window.then(|| cx.add_window(|_, _| OtherWindow));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let (admitted, admission) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        read_request(&mut socket);
        admitted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(8)).unwrap();
        respond(&mut socket, "late close answer");
    });
    let view = open(&path, cx);
    draft(view, "Question before close", cx);
    act(view, Action::CopilotSend, cx);
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    // Trigger the actual should-close callback through GPUI's test platform.
    let mut visual = gpui::VisualTestContext::from_window(view.into(), cx);
    assert!(!visual.simulate_close());
    view.update(cx, |view, _, _| {
        assert!(!view.copilot.closed);
        assert!(view.copilot.worker.physically_active());
        assert_eq!(view.copilot.session.outcome(), &Outcome::Cancelled);
        assert!(
            view.copilot
                .notice
                .as_ref()
                .unwrap()
                .contains("Close again")
        );
    })
    .unwrap();
    release.send(()).unwrap();
    server.join().unwrap();
    finish(view, cx);
    view.update(cx, |view, _, _| {
        assert!(!view.copilot.closed); // Retirement must not auto-close.
        assert_eq!(view.copilot.session.messages().len(), 1);
    })
    .unwrap();
    assert!(visual.simulate_close()); // A fresh native request may now close.
    view.update(cx, |_, window, _| {
        window.remove_window();
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui::test]
fn apply_keeps_memory_change_and_reports_persistence_failure(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        read_request(&mut socket);
        respond(
            &mut socket,
            r#"{"answer":"Add Tokyo.","suggestion":{"timeZoneIDs":["Asia/Tokyo"]}}"#,
        );
    });
    let view = open(&path, cx);
    draft(view, "Add Tokyo", cx);
    act(view, Action::CopilotSend, cx);
    finish(view, cx);
    server.join().unwrap();
    let (id, revision) = view
        .update(cx, |view, _, _| {
            // A directory cannot be loaded/saved as the settings JSON file.
            view.settings_path = folder.path().to_owned();
            (
                view.copilot.session.messages().last().unwrap().id,
                view.planner_revision,
            )
        })
        .unwrap();
    act(view, Action::CopilotApply(id, revision), cx);
    view.update(cx, |view, _, _| {
        assert!(view.planner.zones.iter().any(|z| z.name() == "Asia/Tokyo"));
        assert!(
            view.copilot
                .session
                .messages()
                .last()
                .unwrap()
                .applied_parts
                .contains(Parts::LOCATIONS)
        );
        assert!(view.error.is_some());
        assert!(
            view.copilot
                .notice
                .as_ref()
                .unwrap()
                .contains("preferences were not saved")
        );
    })
    .unwrap();
    assert_eq!(Settings::load(&path).unwrap().zone_ids, ["UTC"]);
}

#[gpui::test]
fn app_quit_refuses_held_visible_and_hidden_work_then_fresh_quit_latches(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    cx.update(crate::shutdown::init);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let (admitted, admission) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        read_request(&mut socket);
        admitted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(8)).unwrap();
        respond(&mut socket, "late cancelled answer");
        listener
    });
    let view = open(&path, cx);
    act(view, Action::Copilot, cx);
    draft(view, "Hold while I try Quit", cx);
    act(view, Action::CopilotSend, cx);
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    cx.update(|cx| cx.dispatch_action(&crate::shutdown::Quit));
    cx.run_until_parked();
    assert!(!cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 0);
    act(view, Action::Copilot, cx);
    cx.update(|cx| cx.dispatch_action(&crate::shutdown::Quit));
    cx.run_until_parked();
    assert!(!cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 0);
    release.send(()).unwrap();
    let listener = server.join().unwrap();
    finish(view, cx);
    assert_eq!(
        cx.read(crate::shutdown::quit_calls),
        0,
        "retirement cannot replay refused Quit"
    );
    let retained = view.update(cx, |_, _, cx| cx.entity()).unwrap();
    cx.update(|cx| cx.dispatch_action(&crate::shutdown::Quit));
    cx.run_until_parked();
    assert!(cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
    // Successful Quit legitimately replaces the native root with Closing.
    // Exercise a retained owner's late callback instead of a stale typed window.
    retained.update(cx, |view, cx| {
        view.copilot.draft.update(cx, |editor, cx| {
            editor.set_text("Must not send after Quit".into(), cx)
        });
        view.send_copilot(false, None, cx);
        assert!(!view.copilot.worker.busy());
        assert_eq!(
            view.copilot.draft.read(cx).text(),
            "Must not send after Quit"
        );
    });
    listener.set_nonblocking(true).unwrap();
    assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
}

#[gpui::test]
fn apply_focus_survives_live_revision_and_disappears_when_not_actionable(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    Settings {
        zone_ids: vec!["UTC".into()],
        anchor_zone_id: "UTC".into(),
        ..Default::default()
    }
    .save(&path)
    .unwrap();
    let view = open(&path, cx);
    act(view, Action::Copilot, cx);
    let (id, focus) = view
        .update(cx, |view, window, cx| {
            let context =
                protocol::Context::from_planner(&view.planner, view.planner.instant, "UTC")
                    .unwrap();
            let started = view
                .copilot
                .session
                .begin("Add Tokyo", context, |_, _| Ok(()))
                .unwrap();
            view.copilot.session.complete(
                started.generation,
                protocol::parse_response(
                    r#"{"answer":"Add Tokyo","suggestion":{"timeZoneIDs":["Asia/Tokyo"]}}"#,
                ),
            );
            let id = view.copilot.session.messages().last().unwrap().id;
            view.prepare_focus(cx);
            let focus = view.action_focus[&Action::CopilotApply(id, view.planner_revision)].clone();
            focus.focus(window);
            assert!(view.tab_order(cx).contains(&focus));
            view.planner.follows_now = true;
            let now = clock::parse_instant("2026-10-08T12:01:00Z").unwrap();
            assert!(view.planner.refresh_now(now).unwrap());
            view.changed_in_day(Ok(()), false, cx);
            view.prepare_focus(cx);
            assert!(view.action_focus[&Action::CopilotApply(id, view.planner_revision)] == focus);
            assert!(focus.is_focused(window));
            (id, focus)
        })
        .unwrap();
    let revision = view.update(cx, |view, _, _| view.planner_revision).unwrap();
    act(view, Action::CopilotApply(id, revision), cx);
    view.update(cx, |view, _, cx| {
        view.prepare_focus(cx);
        assert!(
            !view
                .copilot_actions()
                .iter()
                .any(|a| matches!(a, Action::CopilotApply(_, _)))
        );
        assert!(!view.tab_order(cx).contains(&focus));
    })
    .unwrap();
    act(view, Action::Copilot, cx);
    view.update(cx, |view, _, cx| {
        view.prepare_focus(cx);
        assert!(view.copilot_actions().is_empty());
        assert!(
            !view
                .tab_order(cx)
                .contains(&view.copilot.draft.read(cx).focus_handle(cx))
        );
    })
    .unwrap();
}

#[gpui::test]
fn explicit_apply_feedback_and_failed_request_are_both_visible(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let view = open(&folder.path().join("settings.json"), cx);
    view.update(cx, |view, window, cx| {
        let context =
            protocol::Context::from_planner(&view.planner, view.planner.instant, "UTC").unwrap();
        let first = view
            .copilot
            .session
            .begin("First", context.clone(), |_, _| Ok(()))
            .unwrap();
        view.copilot.session.complete(
            first.generation,
            Ok(protocol::parse_response(
                r#"{"answer":"Move time","suggestion":{"referenceDate":"2026-10-08T14:00:00Z"}}"#,
            )
            .unwrap()),
        );
        let id = view.copilot.session.messages().last().unwrap().id;
        let old_revision = view.planner_revision;
        let second = view
            .copilot
            .session
            .begin("Second", context, |_, _| Ok(()))
            .unwrap();
        view.copilot
            .session
            .complete(second.generation, Err("Synthetic provider failed".into()));
        view.act(Action::Day(1), window, cx);
        view.act(Action::CopilotApply(id, old_revision), window, cx);
        let notices = view.copilot_notices();
        assert!(
            notices
                .iter()
                .any(|n| n.contains("Review the updated suggestion"))
        );
        assert!(notices.iter().any(|n| n == "Synthetic provider failed"));
        view.act(Action::CopilotApply(id, view.planner_revision), window, cx);
        let notices = view.copilot_notices();
        assert!(notices.iter().any(|n| n.starts_with("Applied:")));
        assert!(notices.iter().any(|n| n == "Synthetic provider failed"));
    })
    .unwrap();
}
