//! Synthetic GPUI host coverage, not manual pointer acceptance.
use super::{Settings, SettingsView, SetupAction};
use gpui::{TestAppContext, WindowHandle};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

fn read_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut header_end = None;
    let mut length = 0;
    loop {
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 1);
        bytes.push(byte[0]);
        assert!(bytes.len() < 32_000);
        if header_end.is_none() && bytes.ends_with(b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            length = headers
                .lines()
                .find_map(|l| {
                    l.strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
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
fn finish_setup(view: WindowHandle<SettingsView>, cx: &mut TestAppContext) {
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        let busy = view
            .update(cx, |view, _, _| {
                view.setup.poll();
                view.setup.busy()
            })
            .unwrap();
        if !busy {
            break;
        }
        assert!(Instant::now() < until, "setup worker did not retire");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn settings_load_select_persist_test_then_ask_ai_uses_selected_model(cx: &mut TestAppContext) {
    setup_flow(cx);
}
fn setup_flow(cx: &mut TestAppContext) {
    let _serial = super::provider_setup::TEST_LOCK.lock().unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/v1", server.local_addr().unwrap());
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    let settings = Settings {
        provider_endpoint: endpoint.clone(),
        provider_model: "before".into(),
        ..Settings::default()
    };
    settings.save(&path).unwrap();
    let view = cx.add_window(|window, cx| SettingsView::new_at(path.clone(), window, cx));
    cx.run_until_parked();
    assert!(
        matches!(server.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    view.update(cx, |view, _, cx| {
        view.model
            .update(cx, |editor, cx| editor.set_text("manual edit".into(), cx));
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        matches!(server.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    server.set_nonblocking(false).unwrap();
    let worker = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..3 {
            let (mut stream, address) = server.accept().unwrap();
            assert!(address.ip().is_loopback());
            requests.push(read_request(&mut stream));
            let body = if index == 0 {
                r#"{"data":[{"id":"zeta"},{"id":"selected-model"},{"id":"selected-model"}]}"#
            } else {
                "data: {\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\ndata: [DONE]\n\n"
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
        requests
    });
    view.update(cx, |view, _, cx| {
        view.start_setup_with_key(SetupAction::Load, "fixture-only-key", cx)
    })
    .unwrap();
    finish_setup(view, cx);
    view.update(cx, |view, _, cx| {
        assert_eq!(view.setup.models, ["selected-model", "zeta"]);
        // The same model editor mutation used by the model-menu action.
        view.select_model("selected-model".into(), cx);
        view.edit_generation(super::generation::Edit::TemperatureMode(true), cx);
        view.edit_generation(super::generation::Edit::Temperature(-0.3), cx);
        view.edit_generation(
            super::generation::Edit::Effort(Some(
                bellobox_core::ai::generation::ReasoningEffort::High,
            )),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        Settings::load(&path).unwrap().provider_model,
        "selected-model"
    );
    view.update(cx, |view, _, cx| {
        view.start_setup_with_key(SetupAction::Test, "fixture-only-key", cx)
    })
    .unwrap();
    finish_setup(view, cx);
    view.update(cx, |view, _, _| {
        assert_eq!(
            view.setup.test_message.as_deref(),
            Some("Connected: hello world")
        )
    })
    .unwrap();
    // The actual Ask AI settings resolver/request/streaming transport, without
    // process env or a credential lookup in this fixture.
    let config =
        crate::transport::config_from_settings(Settings::load(&path).unwrap(), None, None, None)
            .unwrap();
    let request =
        bellobox_core::ai::request(&config, "fixture-only-key", "Explain", "fixture selection")
            .unwrap();
    let mut answer = String::new();
    crate::transport::send(request, |chunk| answer.push_str(chunk)).unwrap();
    assert_eq!(answer, "hello world");
    let requests = worker.join().unwrap();
    assert!(requests[0].starts_with("GET /v1/models "));
    for request in &requests[1..] {
        assert!(request.starts_with("POST /v1/chat/completions "));
        let body: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["model"], "selected-model");
        assert_eq!(body["stream"], true);
        assert_eq!(body["temperature"], 0.7);
        assert_eq!(body["reasoning_effort"], "high");
        assert!(body.get("max_completion_tokens").is_none());
    }
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("fixture-only-key")
    );
    view.update(cx, |view, _, cx| {
        view.model
            .update(cx, |editor, cx| editor.set_text("new-model".into(), cx))
    })
    .unwrap();
    cx.run_until_parked();
    view.update(cx, |view, _, _| assert!(view.setup.test_message.is_none()))
        .unwrap();
}

#[gpui::test]
fn real_configuration_callbacks_reject_delayed_setup_results(cx: &mut TestAppContext) {
    check_callbacks(cx);
}
fn check_callbacks(cx: &mut TestAppContext) {
    for action in [SetupAction::Load, SetupAction::Test] {
        for edit in 0..4 {
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("settings.json");
            let settings = Settings {
                provider_endpoint: "http://127.0.0.1:1/v1".into(),
                provider_model: "original".into(),
                ..Default::default()
            };
            settings.save(&path).unwrap();
            let view = cx.add_window(|window, cx| SettingsView::new_at(path.clone(), window, cx));
            cx.run_until_parked();
            let finish = view
                .update(cx, |view, _, _| {
                    view.setup.models = vec!["previous".into()];
                    view.setup.test_message = Some("previous success".into());
                    view.setup.delayed_fixture(action)
                })
                .unwrap();
            view.update(cx, |view, _, cx| match edit {
                0 => view.endpoint.update(cx, |editor, cx| {
                    editor.set_text("http://127.0.0.1:2/v1".into(), cx)
                }),
                1 => view.switch_provider(super::Provider::Anthropic, cx),
                2 => view.set_request_api(true, cx),
                _ => view.select_model("new-selected-model".into(), cx),
            })
            .unwrap();
            cx.run_until_parked();
            finish();
            view.update(cx, |view, window, _| {
                assert!(view.setup.poll());
                assert!(view.setup.test_message.is_none());
                assert!(view.setup.load_message.is_none());
                assert!(!view.setup.models.iter().any(|model| model == "stale-model"));
                if edit < 3 {
                    assert!(view.setup.models.is_empty());
                }
                window.remove_window();
            })
            .unwrap();
            cx.run_until_parked();
        }
    }
}
#[gpui::test]
fn actual_window_close_fences_retained_entity_and_fresh_reopen(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    let view = cx.add_window(|window, cx| SettingsView::new_at(path.clone(), window, cx));
    let retained = view.root(cx).unwrap();
    let finish = view
        .update(cx, |view, _, _| {
            view.setup.delayed_fixture(SetupAction::Test)
        })
        .unwrap();
    view.update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    finish();
    retained.update(cx, |view, cx| {
        assert!(view.setup.poll());
        assert!(view.setup.test_message.is_none());
        let config = bellobox_core::ai::Config {
            provider: bellobox_core::ai::Provider::OpenAIChat,
            endpoint: "http://127.0.0.1:1".into(),
            model: "fixture".into(),
            system_prompt: String::new(),
            max_output_tokens: 20,
            generation_options: Default::default(),
        };
        assert!(
            view.setup
                .start(SetupAction::Test, &config, "fixture-only-key", cx)
                .unwrap_err()
                .contains("closed")
        );
    });
    let fresh = cx.add_window(|window, cx| SettingsView::new_at(path, window, cx));
    fresh
        .update(cx, |view, _, _| assert!(view.setup.test_message.is_none()))
        .unwrap();
}

#[gpui::test]
fn actual_close_reopen_cannot_restart_a_held_transport(cx: &mut TestAppContext) {
    held_window_flow(cx);
}
fn held_window_flow(cx: &mut TestAppContext) {
    let _serial = super::provider_setup::TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    Settings {
        provider_endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        provider_model: "fixture".into(),
        ..Default::default()
    }
    .save(&path)
    .unwrap();
    let (admitted, admission) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let request = read_request(&mut socket);
        assert!(request.starts_with("GET /v1/models "));
        admitted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(5)).unwrap();
        let body = r#"{"data":[{"id":"stale-model"}]}"#;
        let _ = write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
    });
    let old = cx.add_window(|window, cx| SettingsView::new_at(path.clone(), window, cx));
    let retained = old.root(cx).unwrap();
    old.update(cx, |view, _, cx| {
        view.start_setup_with_key(SetupAction::Load, "fixture-only-key", cx)
    })
    .unwrap();
    admission.recv_timeout(Duration::from_secs(5)).unwrap();
    old.update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let fresh = cx.add_window(|window, cx| SettingsView::new_at(path, window, cx));
    fresh
        .update(cx, |view, _, cx| {
            view.start_setup_with_key(SetupAction::Load, "fixture-only-key", cx);
            assert!(
                view.setup
                    .load_message
                    .as_ref()
                    .unwrap()
                    .contains("still stopping")
            );
        })
        .unwrap();
    release.send(()).unwrap();
    worker.join().unwrap();
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
    retained.update(cx, |view, _| assert!(view.setup.models.is_empty()));
    fresh
        .update(cx, |view, _, _| assert!(view.setup.models.is_empty()))
        .unwrap();
}
