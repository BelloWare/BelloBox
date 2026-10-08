use super::Launcher;
use gpui::{EntityInputHandler, Focusable, KeyDownEvent, Keystroke, TestAppContext};
fn key(name: &str, held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(name).unwrap(),
        is_held: held,
    }
}
#[gpui::test]
fn actual_launcher_ime_escape_and_held_escape_do_not_close_or_steal_composition(
    cx: &mut TestAppContext,
) {
    let view = cx.add_window(|window, cx| Launcher::new("2026-10-08T12:00:00Z".into(), window, cx));
    view.update(cx, |v, window, cx| {
        v.query
            .update(cx, |e, cx| e.set_text("world clock".into(), cx));
        v.refresh_preview(window, cx);
        let clock = v.clock.as_ref().unwrap().clone();
        let draft = clock.read(cx).draft_editor();
        draft.read(cx).focus_handle(cx).focus(window);
        draft.update(cx, |e, cx| {
            e.replace_and_mark_text_in_range(None, "に", Some(1..1), window, cx)
        });
        assert!(v.route_clock_editor_key(&key("escape", false), window, cx));
        assert!(draft.read(cx).has_marked_text());
        assert!(draft.read(cx).focus_handle(cx).is_focused(window));
        assert!(!v.suppress_escape);
        draft.update(cx, |e, cx| e.replace_text_in_range(None, "日", window, cx));
        assert!(v.route_clock_editor_key(&key("escape", false), window, cx));
        assert!(v.query.read(cx).focus_handle(cx).is_focused(window));
        assert!(v.suppress_escape);
        assert!(v.route_clock_editor_key(&key("escape", true), window, cx));
        assert!(v.query.read(cx).focus_handle(cx).is_focused(window));
    })
    .unwrap();
}

use crate::clock_copilot_worker::{RetirementGuard, TEST_LOCK};
use bellobox_core::settings::Settings;
use gpui::WindowHandle;
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

struct Held {
    release: mpsc::Sender<()>,
    server: std::thread::JoinHandle<()>,
    guard: RetirementGuard,
    _folder: tempfile::TempDir,
}
fn held_launcher(cx: &mut TestAppContext) -> (WindowHandle<Launcher>, Held) {
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
        respond(&mut socket, "Late launcher answer");
    });
    let view = cx.add_window(|window, cx| Launcher::new("2026-10-08T12:00:00Z".into(), window, cx));
    let guard = view
        .update(cx, |v, window, cx| {
            v.settings = Settings::load(&path).unwrap();
            v.settings_writable = false;
            v.query
                .update(cx, |e, cx| e.set_text("world clock".into(), cx));
            v.refresh_preview(window, cx);
            v.clock.as_ref().unwrap().update(cx, |clock, cx| {
                clock.send_fixture(&path, "Held source question", cx)
            });
            v.clock_guards.borrow()[0].clone()
        })
        .unwrap();
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    (
        view,
        Held {
            release,
            server,
            guard,
            _folder: folder,
        },
    )
}
fn release(held: Held) {
    held.release.send(()).unwrap();
    held.server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while held.guard.physically_active() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}
struct Other;
impl gpui::Render for Other {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
    }
}

#[gpui::test]
fn native_close_keeps_guard_across_discard_replacement_and_non_clock_selection(
    cx: &mut TestAppContext,
) {
    let _serial = TEST_LOCK.lock().unwrap();
    let _other = cx.add_window(|_, _| Other);
    let (view, held) = held_launcher(cx);
    let retained = view
        .update(cx, |v, window, cx| {
            let retained = v.clock.as_ref().unwrap().clone();
            v.discard_clock(cx);
            v.input = "2026-10-09T11:00:00Z".into();
            v.refresh_preview(window, cx);
            assert!(v.clock.is_some());
            assert!(
                v.clock_guards
                    .borrow()
                    .iter()
                    .any(RetirementGuard::physically_active)
            );
            v.query.update(cx, |e, cx| e.set_text("uuid".into(), cx));
            v.refresh_preview(window, cx);
            assert!(!v.clock_active);
            retained
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(view.into(), cx);
    assert!(!visual.simulate_close());
    view.update(cx, |v, _, _| {
        assert!(v.notice.as_deref().unwrap().contains("Stopping"))
    })
    .unwrap();
    // A retained stale preview cannot revive its cancelled request.
    retained.update(cx, |clock, cx| {
        clock.send_fixture(
            &held._folder.path().join("settings.json"),
            "Must not send",
            cx,
        )
    });
    assert!(held.guard.physically_active());
    release(held);
    assert!(visual.simulate_close());
}

#[gpui::test]
fn deactivation_uses_launcher_close_gate_instead_of_dropping_live_worker(cx: &mut TestAppContext) {
    let _serial = TEST_LOCK.lock().unwrap();
    let other = cx.add_window(|_, _| Other);
    let (view, held) = held_launcher(cx);
    view.update(cx, |v, window, _| {
        v.was_active = true;
        window.activate_window();
    })
    .unwrap();
    cx.run_until_parked();
    other
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    view.update(cx, |v, _, _| {
        assert!(v.clock_active);
        assert!(v.notice.as_deref().unwrap().contains("Stopping"));
        assert!(
            v.clock_guards
                .borrow()
                .iter()
                .any(RetirementGuard::physically_active)
        );
    })
    .unwrap();
    release(held);
    // Retirement never replays a refused deactivation-close automatically.
    view.update(cx, |v, _, _| assert!(v.clock.is_some()))
        .unwrap();
}

#[gpui::test]
fn actual_launch_failure_preserves_source_then_success_transfers_orphaned_guard(
    cx: &mut TestAppContext,
) {
    let _serial = TEST_LOCK.lock().unwrap();
    let _other = cx.add_window(|_, _| Other);
    let (view, held) = held_launcher(cx);
    view.update(cx, |v, window, cx| {
        v.clock
            .as_ref()
            .unwrap()
            .update(cx, |clock, _| clock.invalidate_handoff_fixture());
        v.launch(window, cx);
        assert!(v.notice.as_ref().unwrap().contains("years"));
        assert!(!v.clock_transferred);
        assert!(v.clock.is_some());
        assert!(
            v.clock_guards
                .borrow()
                .iter()
                .any(RetirementGuard::physically_active)
        );
        v.discard_clock(cx);
        v.input = "2026-10-09T11:00:00Z".into();
        v.refresh_preview(window, cx);
        // The current preview has no worker. Its predecessor's guard must travel.
        v.launch(window, cx);
        assert!(v.clock_transferred);
    })
    .unwrap();
    cx.run_until_parked();
    let target = cx.read(|cx| {
        cx.windows()
            .into_iter()
            .find_map(|w| w.downcast::<crate::world_clock_ui::WorldClock>())
            .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(target.into(), cx);
    assert!(!visual.simulate_close());
    assert!(held.guard.physically_active());
    assert!(
        view.update(cx, |_, _, _| ()).is_err(),
        "successful transfer closes source"
    );
    release(held);
    assert!(visual.simulate_close());
}

#[gpui::test]
fn launcher_handoff_draft_limit_counts_utf8_bytes_without_truncating_source(
    cx: &mut TestAppContext,
) {
    let view = cx.add_window(|window, cx| Launcher::new("2026-10-08T12:00:00Z".into(), window, cx));
    view.update(cx, |v, window, cx| {
        v.query
            .update(cx, |e, cx| e.set_text("world clock".into(), cx));
        v.refresh_preview(window, cx);
        let clock = v.clock.as_ref().unwrap().clone();
        let draft = clock.read(cx).draft_editor();
        // Both scalar-heavy and multibyte strings at the exact byte cap transfer whole.
        for text in ["x".repeat(8192), "界".repeat(2730) + "ab"] {
            assert_eq!(text.len(), 8192);
            draft.update(cx, |e, cx| e.set_text(text.clone(), cx));
            let handoff = clock.read(cx).handoff(cx).unwrap();
            assert_eq!(handoff.draft, text);
            assert_eq!(draft.read(cx).text(), text);
        }
        let text = "界".repeat(2731);
        assert_eq!(text.len(), 8193);
        assert!(text.chars().count() < 8192);
        draft.update(cx, |e, cx| e.set_text(text.clone(), cx));
        let windows = cx.windows().len();
        v.launch(window, cx);
        assert_eq!(cx.windows().len(), windows);
        assert!(
            cx.windows()
                .iter()
                .all(|w| w.downcast::<crate::world_clock_ui::WorldClock>().is_none())
        );
        assert_eq!(draft.read(cx).text(), text);
        assert_eq!(v.clock.as_ref().unwrap(), &clock);
        assert!(!v.clock_transferred);
        assert!(v.notice.as_ref().unwrap().contains("8,192-byte"));
    })
    .unwrap();
}

#[gpui::test]
fn command_k_returns_to_search_without_sending_or_losing_draft(cx: &mut TestAppContext) {
    let view = cx.add_window(|window, cx| Launcher::new("2026-10-08T12:00:00Z".into(), window, cx));
    view.update(cx, |v, window, cx| {
        v.query
            .update(cx, |e, cx| e.set_text("world clock".into(), cx));
        v.refresh_preview(window, cx);
        let clock = v.clock.as_ref().unwrap().clone();
        let draft = clock.read(cx).draft_editor();
        draft.update(cx, |e, cx| e.set_text("keep unsent".into(), cx));
        for shortcut in ["ctrl-k", "cmd-k"] {
            draft.read(cx).focus_handle(cx).focus(window);
            let event = key(shortcut, false);
            assert!(!v.route_clock_editor_key(&event, window, cx));
            assert!(!clock.update(cx, |clock, cx| clock.handle_key(&event, window, cx)));
            assert!(v.clock_search_shortcut(&event, window, cx));
            assert!(v.query.read(cx).focus_handle(cx).is_focused(window));
            assert_eq!(v.query.read(cx).text(), "");
            assert_eq!(draft.read(cx).text(), "keep unsent");
        }
    })
    .unwrap();
}
