//! App-controlled Quit coverage only; no native or last-window policy claim.
use super::{SettingsView, SetupAction};
use bellobox_core::settings::Settings;
use gpui::{Context, Render, TestAppContext, Window};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

struct Home;
impl Render for Home {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        gpui::div()
    }
}
fn read_request(socket: &mut std::net::TcpStream) {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut end = None;
    let mut length = 0;
    loop {
        let mut byte = [0];
        assert_eq!(socket.read(&mut byte).unwrap(), 1);
        bytes.push(byte[0]);
        assert!(bytes.len() < 32_000);
        if end.is_none() && bytes.ends_with(b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            length = head
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length:")
                        .map(|n| n.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            end = Some(bytes.len());
        }
        if end.is_some_and(|n| bytes.len() == n + length) {
            break;
        }
    }
}
#[gpui::test]
fn active_model_load_blocks_app_quit_through_settings_close(cx: &mut TestAppContext) {
    held_quit(cx, SetupAction::Load);
}
#[gpui::test]
fn active_connection_test_blocks_app_quit_through_settings_close(cx: &mut TestAppContext) {
    held_quit(cx, SetupAction::Test);
}
fn held_quit(cx: &mut TestAppContext, action: SetupAction) {
    let _serial = super::provider_setup::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cx.update(crate::shutdown::init);
    let home = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        Home
    });
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    Settings {
        provider_endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        provider_model: "fixture-model".into(),
        ..Default::default()
    }
    .save(&path)
    .unwrap();
    let (admitted, admission) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        read_request(&mut socket);
        admitted.send(()).unwrap();
        let _ = released.recv_timeout(Duration::from_secs(10));
        let body = match action {
            SetupAction::Load => r#"{"data":[{"id":"late-model"}]}"#,
            SetupAction::Test => {
                "data: {\"choices\":[{\"delta\":{\"content\":\"late hello\"}}]}\n\ndata: [DONE]\n\n"
            }
        };
        let _ = write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
    });
    let settings = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        SettingsView::new_at(path, window, cx)
    });
    let retained = settings.root(cx).unwrap();
    settings
        .update(cx, |view, _, cx| {
            view.start_setup_with_key(action, "fixture-only-key", cx)
        })
        .unwrap();
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut visual = gpui::VisualTestContext::from_window(settings.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-q"
    } else {
        "ctrl-q"
    });
    assert!(
        !cx.read(crate::shutdown::requested),
        "app Quit must refuse while setup transport is physically held"
    );
    assert_eq!(cx.read(crate::shutdown::quit_calls), 0);
    assert!(crate::shutdown::pending_refusal(cx).is_some());
    crate::shutdown::dismiss_refusal(cx);
    settings
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    cx.dispatch_action(home.into(), crate::shutdown::Quit);
    assert!(!cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 0);
    assert!(crate::shutdown::pending_refusal(cx).is_some());
    crate::shutdown::dismiss_refusal(cx);
    release.send(()).unwrap();
    server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let busy = retained.update(cx, |view, _| {
            view.setup.poll();
            view.setup.busy()
        });
        if !busy {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    retained.update(cx, |view, _| {
        assert!(view.setup.models.is_empty());
        assert!(view.setup.test_message.is_none());
        assert!(!view.setup.poll());
    });
    assert!(
        !cx.read(crate::shutdown::requested),
        "old refused Quit must never resume itself"
    );
    cx.dispatch_action(home.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[gpui::test]
fn invalid_test_request_leaves_no_app_quit_blocker(cx: &mut TestAppContext) {
    rejected_start(cx, false);
}
#[gpui::test]
fn failed_worker_spawn_releases_both_admission_leases(cx: &mut TestAppContext) {
    rejected_start(cx, true);
}
fn rejected_start(cx: &mut TestAppContext, fail_spawn: bool) {
    let _serial = super::provider_setup::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cx.update(crate::shutdown::init);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    Settings {
        provider_endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        provider_model: if fail_spawn {
            "fixture-model".into()
        } else {
            String::new()
        },
        ..Default::default()
    }
    .save(&path)
    .unwrap();
    let settings = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        SettingsView::new_at(path, window, cx)
    });
    settings
        .update(cx, |view, _, cx| {
            view.setup.fail_next_spawn = fail_spawn;
            view.start_setup_with_key(SetupAction::Test, "fixture-only-key", cx);
            assert!(!view.setup.busy());
            let error = view.setup.test_message.as_ref().unwrap();
            assert!(error.contains(if fail_spawn {
                "Cannot start provider setup worker"
            } else {
                "Choose a model"
            }));
            if fail_spawn {
                // Reacquiring and failing another spawn must not report Busy.
                view.setup.fail_next_spawn = true;
                view.start_setup_with_key(SetupAction::Test, "fixture-only-key", cx);
                assert!(
                    view.setup
                        .test_message
                        .as_ref()
                        .unwrap()
                        .contains("Cannot start provider setup worker")
                );
            }
        })
        .unwrap();
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    cx.dispatch_action(settings.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[gpui::test]
fn shutdown_latch_rejects_retained_settings_load_before_network(cx: &mut TestAppContext) {
    late_admission(cx, SetupAction::Load);
}
#[gpui::test]
fn shutdown_latch_rejects_retained_settings_test_before_network(cx: &mut TestAppContext) {
    late_admission(cx, SetupAction::Test);
}
fn late_admission(cx: &mut TestAppContext, action: SetupAction) {
    let _serial = super::provider_setup::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cx.update(crate::shutdown::init);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    Settings {
        provider_endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        provider_model: "fixture-model".into(),
        ..Default::default()
    }
    .save(&path)
    .unwrap();
    let settings = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        SettingsView::new_at(path, window, cx)
    });
    let retained = settings.root(cx).unwrap();
    // Retain the app's existing physical drain without any native media work.
    let ticket = cx.update(|cx| crate::shutdown::registry(cx).admit(|| {}).unwrap());
    cx.dispatch_action(settings.into(), crate::shutdown::Quit);
    assert!(cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 0);
    retained.update(cx, |view, cx| {
        view.start_setup_with_key(action, "fixture-only-key", cx)
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut connected = false;
    loop {
        if let Ok((mut socket, _)) = listener.accept() {
            connected = true;
            read_request(&mut socket);
            let body = match action {
                SetupAction::Load => r#"{"data":[]}"#,
                SetupAction::Test => {
                    "data: {\"choices\":[{\"delta\":{\"content\":\"unexpected\"}}]}\n\ndata: [DONE]\n\n"
                }
            };
            let _ = write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        }
        let busy = retained.update(cx, |view, _| {
            view.setup.poll();
            view.setup.busy()
        });
        if !busy {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !connected,
        "shutdown-latched Settings must not dispatch a new HTTP request"
    );
    retained.update(cx, |view, _| {
        let message = match action {
            SetupAction::Load => &view.setup.load_message,
            SetupAction::Test => &view.setup.test_message,
        };
        assert!(message.as_ref().unwrap().contains("closing"));
        assert!(!view.setup.busy());
    });
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    drop(ticket);
    cx.run_until_parked();
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}
