//! Actual GPUI dispatch with synthetic numeric-loopback HTTP only.
use super::{Action, LauncherClockPreview, RetirementGuard, Settings, apply, clock, protocol};
use crate::clock_copilot_worker::TEST_LOCK;
use bellobox_core::clock::copilot::Parts;
use gpui::{Focusable, TestAppContext, WindowHandle};
use std::{cell::RefCell, rc::Rc};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::{Duration, Instant},
};
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
                // macOS inherits the listener's nonblocking mode on accept.
                socket.set_nonblocking(false).unwrap();
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
#[test]
fn fixture_reader_waits_for_delayed_request_bytes() {
    const HEADERS: &str = "POST /fixture HTTP/1.1\r\nContent-Length: 4\r\n\r\n";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (started, reading) = mpsc::channel();
    let (finished, result) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        started.send(()).unwrap();
        let request = read_request(&mut socket);
        assert_eq!(socket.read_timeout().unwrap(), Some(Duration::from_secs(5)));
        finished.send(request).unwrap();
    });
    reading.recv_timeout(Duration::from_secs(5)).unwrap();
    // No bytes are available yet; a nonblocking accepted socket fails here.
    assert_eq!(
        result.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    client.write_all(HEADERS.as_bytes()).unwrap();
    // Receiving the headers must still wait for the delayed request body.
    assert_eq!(
        result.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    client.write_all(b"test").unwrap();
    assert_eq!(
        result.recv_timeout(Duration::from_secs(5)).unwrap(),
        format!("{HEADERS}test")
    );
    server.join().unwrap();
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

fn open(path: &std::path::Path, cx: &mut TestAppContext) -> WindowHandle<LauncherClockPreview> {
    cx.add_window(|window, cx| {
        let settings = Settings::load(path).unwrap();
        let mut v = LauncherClockPreview::new("2026-10-08T12:00:00Z".into(), &settings, window, cx);
        v.copilot.settings_path = path.to_owned();
        v
    })
}
fn act(view: WindowHandle<LauncherClockPreview>, action: Action, cx: &mut TestAppContext) {
    view.update(cx, |v, _, cx| v.copilot_action(action, cx))
        .unwrap();
}
fn draft(view: WindowHandle<LauncherClockPreview>, text: &str, cx: &mut TestAppContext) {
    view.update(cx, |v, _, cx| {
        v.copilot
            .draft
            .update(cx, |e, cx| e.set_text(text.into(), cx))
    })
    .unwrap();
}
fn finish(view: WindowHandle<LauncherClockPreview>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let busy = view
            .update(cx, |v, _, _| {
                if let Some((g, r)) = v.copilot.worker.poll() {
                    v.copilot.session.complete(g, r);
                }
                v.copilot.worker.busy()
            })
            .unwrap();
        if !busy {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[gpui::test]
fn palette_send_time_apply_handoff_preserves_deferred_locations_and_draft(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let before = std::fs::read(&path).unwrap();
    let view = open(&path, cx);
    draft(view, "Find tomorrow and Tokyo", cx);
    cx.run_until_parked();
    assert!(listener.accept().is_err());
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        let request = read_request(&mut socket);
        respond(
            &mut socket,
            r#"{"answer":"Try tomorrow.","suggestion":{"referenceDate":"2026-10-09T13:00:00Z","timeZoneIDs":["Asia/Tokyo"],"anchorTimeZoneID":"Asia/Tokyo"}}"#,
        );
        request
    });
    act(view, Action::Send, cx);
    finish(view, cx);
    assert!(server.join().unwrap().contains("Find tomorrow and Tokyo"));
    view.update(cx, |v, _, cx| {
        assert_eq!(v.copilot.session.messages().len(), 2);
        let id = v.copilot.session.messages()[1].id;
        v.copilot_action(Action::Apply(id, v.revision), cx);
        assert_eq!(v.session.planner.time_input(), "13:00");
        assert!(
            !v.session
                .planner
                .zones
                .iter()
                .any(|z| z.name() == "Asia/Tokyo")
        );
        assert_eq!(v.copilot.session.messages()[1].applied_parts, Parts::TIME);
    })
    .unwrap();
    draft(view, "unsent followup", cx);
    let handoff = view.update(cx, |v, _, cx| v.handoff(cx)).unwrap().unwrap();
    // Real target adoption is separately tested; this verifies source transfer and
    // subsequent full-scope planning retain the unapplied location proposal.
    let mut full = clock::Planner::default();
    handoff.apply(&mut full).unwrap();
    assert_eq!(full.date_input(), "2026-10-09");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    view.update(cx, |v, _, cx| {
        assert_eq!(v.copilot.draft.read(cx).text(), "unsent followup");
        let m = &v.copilot.session.messages()[1];
        let plan = apply::prepare(
            &full,
            m.suggestion.as_ref().unwrap(),
            m.applied_parts,
            true,
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.plan.parts, Parts::LOCATIONS);
        plan.commit(&mut full).unwrap();
        assert_eq!(full.reference.name(), "Asia/Tokyo");
    })
    .unwrap();
}
#[gpui::test]
fn discarded_palette_retains_physical_guard_and_rejects_late_actions(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    safe_environment();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    settings(&path, &listener);
    let (entered, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut socket = accept(&listener);
        read_request(&mut socket);
        entered.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(8)).unwrap();
        respond(&mut socket, "old result");
    });
    let view = open(&path, cx);
    let guards = Rc::new(RefCell::new(Vec::new()));
    view.update(cx, |v, _, _| v.retain_guards(guards.clone()))
        .unwrap();
    draft(view, "held", cx);
    act(view, Action::Send, cx);
    ready.recv_timeout(Duration::from_secs(8)).unwrap();
    view.update(cx, |v, _, cx| v.retire_copilot(cx)).unwrap();
    assert!(
        guards
            .borrow()
            .iter()
            .any(RetirementGuard::physically_active)
    );
    draft(view, "must not send", cx);
    act(view, Action::Send, cx);
    assert_eq!(
        view.update(cx, |v, _, _| v.copilot.session.messages().len())
            .unwrap(),
        1
    );
    release.send(()).unwrap();
    finish(view, cx);
    server.join().unwrap();
    assert!(
        !guards
            .borrow()
            .iter()
            .any(RetirementGuard::physically_active)
    );
    assert_eq!(
        view.update(cx, |v, _, _| v.copilot.session.messages().len())
            .unwrap(),
        1
    );
}

#[gpui::test]
fn stale_apply_clear_and_row_switch_preserve_only_current_conversation(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    Settings::default().save(&path).unwrap();
    let view = open(&path, cx);
    view.update(cx, |v, _, cx| {
        let context =
            protocol::Context::from_planner(&v.session.planner, clock::current_time(), "UTC")
                .unwrap();
        let started = v
            .copilot
            .session
            .begin("next day", context, |_, _| Ok(()))
            .unwrap();
        v.copilot.session.complete(
            started.generation,
            protocol::parse_response(
                r#"{"answer":"Tomorrow","suggestion":{"referenceDate":"2026-10-09T13:00:00Z"}}"#,
            ),
        );
        let id = v.copilot.session.messages()[1].id;
        let old_revision = v.revision;
        v.nudge(1, false, cx);
        let before = v.session.planner.instant;
        v.copilot_action(Action::Apply(id, old_revision), cx);
        assert_eq!(v.session.planner.instant, before);
        v.set_active(false, cx);
        v.set_active(true, cx);
        assert_eq!(v.copilot.session.messages().len(), 2);
        v.copilot
            .draft
            .update(cx, |e, cx| e.set_text("keep draft".into(), cx));
        v.copilot_action(Action::Clear, cx);
        assert!(v.copilot.session.messages().is_empty());
        assert_eq!(v.copilot.draft.read(cx).text(), "keep draft");
        v.copilot_action(Action::Apply(id, v.revision), cx);
        assert_eq!(v.session.planner.instant, before);
    })
    .unwrap();
}

#[gpui::test]
fn secondary_editor_owns_enter_arrows_and_escape_never_handoffs(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    Settings::default().save(&path).unwrap();
    let view = open(&path, cx);
    view.update(cx, |v, window, cx| {
        v.copilot.draft.read(cx).focus_handle(cx).focus(window);
        for key in ["up", "down", "left", "right"] {
            let event = gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse(key).unwrap(),
                is_held: false,
            };
            assert!(v.handle_key(&event, window, cx));
        }
        let event = gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("escape").unwrap(),
            is_held: false,
        };
        assert!(!v.handle_key(&event, window, cx));
        // Empty Return is an explicit rejected Send; it never opens a target.
        let event = gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("enter").unwrap(),
            is_held: false,
        };
        assert!(v.handle_key(&event, window, cx));
        assert!(v.copilot.session.messages().is_empty());
        assert!(!v.copilot.worker.busy());
    })
    .unwrap();
}

#[gpui::test]
fn palette_apply_focus_survives_revision_and_tab_reaches_send(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    Settings::default().save(&path).unwrap();
    let view = open(&path, cx);
    view.update(cx, |v, window, cx| {
        let context =
            protocol::Context::from_planner(&v.session.planner, clock::current_time(), "UTC")
                .unwrap();
        let run = v
            .copilot
            .session
            .begin("next day", context, |_, _| Ok(()))
            .unwrap();
        v.copilot.session.complete(
            run.generation,
            protocol::parse_response(
                r#"{"answer":"Tomorrow","suggestion":{"referenceDate":"2026-10-09T13:00:00Z"}}"#,
            ),
        );
        let id = v.copilot.session.messages()[1].id;
        v.refresh_copilot_controls(cx);
        let focus = v.copilot.controls[&Action::Apply(id, 0)].clone();
        focus.focus(window);
        v.nudge(1, false, cx);
        v.refresh_copilot_controls(cx);
        assert!(v.copilot.controls[&Action::Apply(id, 0)].is_focused(window));
        let enter = gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("enter").unwrap(),
            is_held: false,
        };
        assert!(v.handle_key(&enter, window, cx));
        assert_eq!(v.session.planner.date_input(), "2026-10-09");
        v.refresh_copilot_controls(cx);
        assert!(!v.copilot.controls.contains_key(&Action::Apply(id, 0)));
        v.copilot.draft_focus.focus(window);
        let tab = gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("tab").unwrap(),
            is_held: false,
        };
        assert!(v.handle_key(&tab, window, cx));
        assert!(v.copilot.controls[&Action::Send].is_focused(window));
        let back = gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("shift-tab").unwrap(),
            is_held: false,
        };
        assert!(v.handle_key(&back, window, cx));
        assert!(v.copilot.draft_focus.is_focused(window));
    })
    .unwrap();
}
