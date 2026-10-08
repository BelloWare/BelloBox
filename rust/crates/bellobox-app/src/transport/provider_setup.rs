//! Explicit setup operations. Caller owns worker lifetime and stale-result rejection.
use bellobox_core::ai::{
    Request,
    provider_setup::{MAX_MODELS_BYTES, ModelsRequest, parse_models},
};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub fn load_models(request: ModelsRequest, cancel: Arc<AtomicBool>) -> Result<Vec<String>, String> {
    load_models_with_timeout(request, cancel, Duration::from_secs(30))
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Request cancelled.".into())
    } else {
        Ok(())
    }
}
fn load_models_with_timeout(
    request: ModelsRequest,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<Vec<String>, String> {
    cancelled(&cancel)?;
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5).min(timeout))
        .timeout(timeout)
        .build()
        .map_err(|_| "Cannot initialize HTTPS transport.")?;
    let mut builder = client.get(request.url);
    for (name, value) in request.headers {
        builder = builder.header(name, value);
    }
    cancelled(&cancel)?;
    let mut response = builder
        .send()
        .map_err(|_| "Model list connection failed or timed out. Enter a model manually.")?;
    cancelled(&cancel)?;
    if !response.status().is_success() {
        return Err(format!(
            "Model list returned HTTP {}. No redirect was followed. Enter a model manually.",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_MODELS_BYTES as u64)
    {
        return Err("Model list exceeds 1 MiB. Enter a model manually.".into());
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        cancelled(&cancel)?;
        let n = response
            .read(&mut buffer)
            .map_err(|_| "Model list response interrupted or timed out. Enter a model manually.")?;
        cancelled(&cancel)?;
        if n == 0 {
            break;
        }
        if bytes.len() + n > MAX_MODELS_BYTES {
            return Err("Model list exceeds 1 MiB. Enter a model manually.".into());
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    let result = parse_models(&bytes)?;
    cancelled(&cancel)?;
    Ok(result)
}

pub fn test_connection(request: Request, cancel: Arc<AtomicBool>) -> Result<String, String> {
    // Keep only a small preview. The shared streaming transport separately bounds
    // total response bytes and lifetime, and must finish before the worker releases.
    let mut preview = String::new();
    super::send_cancellable(
        request,
        |text| {
            let remaining = 128usize.saturating_sub(preview.chars().count());
            preview.extend(text.chars().take(remaining));
        },
        cancel.clone(),
    )?;
    cancelled(&cancel)?;
    let preview: String = preview.trim().chars().take(60).collect();
    Ok(if preview.is_empty() {
        "Connected".into()
    } else {
        preview
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bellobox_core::ai::{
        Config, Provider,
        provider_setup::{TEST_USER_TEXT, models_request, test_request},
    };
    use std::{io::Write, net::TcpListener, thread};

    fn fixture(response: String) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .and_then(|s| s.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(request).unwrap()
        });
        (url, worker)
    }
    fn reply(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }
    fn flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn loads_models_with_exact_get_and_synthetic_auth() {
        let (url, worker) = fixture(reply(r#"{"data":[{"id":"z"},{"id":"a"},{"id":"z"}]}"#));
        let request =
            models_request(Provider::OpenAIResponses, &url, "synthetic-test-key").unwrap();
        assert_eq!(load_models(request, flag()).unwrap(), ["a", "z"]);
        let captured = worker.join().unwrap().to_lowercase();
        assert!(captured.starts_with("get /v1/models http/1.1\r\n"));
        assert!(captured.contains("authorization: bearer synthetic-test-key"));
    }
    #[test]
    fn refuses_redirect_and_never_echoes_body() {
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        destination.set_nonblocking(true).unwrap();
        let (url, worker) = fixture(format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{}/leak\r\nContent-Length: 15\r\nConnection: close\r\n\r\nprivate-fixture",
            destination.local_addr().unwrap()
        ));
        let err = load_models(
            models_request(Provider::OpenAIChat, &url, "synthetic").unwrap(),
            flag(),
        )
        .unwrap_err();
        assert!(err.contains("302"));
        assert!(!err.contains("private-fixture"));
        worker.join().unwrap();
        // A denied redirect must never deliver either the request or its key.
        assert_eq!(
            destination.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn ambient_proxy_trap_receives_no_setup_requests() {
        let trap = TcpListener::bind("127.0.0.1:0").unwrap();
        trap.set_nonblocking(true).unwrap();
        let proxy = format!("http://{}", trap.local_addr().unwrap());
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "transport::provider_setup::tests::ambient_proxy_child",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .env("BELLOBOX_SETUP_PROXY_CHILD", "synthetic-proxy-fixture")
            .env("HTTP_PROXY", &proxy)
            .env("http_proxy", &proxy)
            .env("HTTPS_PROXY", &proxy)
            .env("https_proxy", &proxy)
            .env("ALL_PROXY", &proxy)
            .env("all_proxy", &proxy)
            .env("NO_PROXY", "")
            .env("no_proxy", "")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        // Dynamic-loader paths are infrastructure, never provider credentials.
        // Preserve only these if the isolated host needs non-system libraries.
        for name in [
            "LD_LIBRARY_PATH",
            "DYLD_LIBRARY_PATH",
            "DYLD_FALLBACK_LIBRARY_PATH",
        ] {
            if let Some(value) = std::env::var_os(name) {
                child.env(name, value);
            }
        }
        let mut child = child.spawn().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let timed_out = loop {
            if child.try_wait().unwrap().is_some() {
                break false;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                break true;
            }
            thread::sleep(Duration::from_millis(10));
        };
        let output = child.wait_with_output().unwrap();
        assert!(!timed_out, "isolated setup proxy fixture timed out");
        assert!(
            output.status.success(),
            "isolated setup proxy fixture failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "isolated child did not execute its fixture"
        );
        assert_eq!(
            trap.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    #[ignore = "runs only in the environment-cleared proxy test subprocess"]
    fn ambient_proxy_child() {
        assert_eq!(
            std::env::var("BELLOBOX_SETUP_PROXY_CHILD").as_deref(),
            Ok("synthetic-proxy-fixture")
        );
        let (url, models_worker) = fixture(reply(r#"{"data":[{"id":"synthetic-model"}]}"#));
        assert_eq!(
            load_models(
                models_request(Provider::OpenAIChat, &url, "synthetic-proxy-key").unwrap(),
                flag()
            )
            .unwrap(),
            ["synthetic-model"]
        );
        let captured = models_worker.join().unwrap().to_lowercase();
        assert!(captured.starts_with("get /v1/models http/1.1\r\n"));
        assert!(captured.contains("authorization: bearer synthetic-proxy-key"));
        let (url, test_worker) = fixture(reply(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello!\"}}]}\n\ndata: [DONE]\n\n",
        ));
        let config = Config {
            provider: Provider::OpenAIChat,
            endpoint: url,
            model: "synthetic-model".into(),
            system_prompt: "synthetic system".into(),
            max_output_tokens: 4096,
        };
        assert_eq!(
            test_connection(
                test_request(&config, "synthetic-proxy-key").unwrap(),
                flag()
            )
            .unwrap(),
            "Hello!"
        );
        let captured = test_worker.join().unwrap().to_lowercase();
        assert!(captured.starts_with("post /v1/chat/completions http/1.1\r\n"));
        assert!(captured.contains("authorization: bearer synthetic-proxy-key"));
    }
    #[test]
    fn bounds_body_and_rejects_malformed() {
        for response in [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_MODELS_BYTES + 1
            ),
            reply("{bad"),
        ] {
            let (url, worker) = fixture(response);
            assert!(
                load_models(
                    models_request(Provider::OpenAIChat, &url, "").unwrap(),
                    flag()
                )
                .is_err()
            );
            worker.join().unwrap();
        }
    }
    #[test]
    fn pre_cancel_does_not_connect() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let request = models_request(
            Provider::OpenAIChat,
            &format!("http://{}", listener.local_addr().unwrap()),
            "",
        )
        .unwrap();
        assert!(
            load_models(request, Arc::new(AtomicBool::new(true)))
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn streaming_test_uses_exact_user_text() {
        let (url, worker) = fixture(reply(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello!\"}}]}\n\ndata: [DONE]\n\n",
        ));
        let c = Config {
            provider: Provider::OpenAIChat,
            endpoint: url,
            model: "synthetic-model".into(),
            system_prompt: "synthetic system".into(),
            max_output_tokens: 4096,
        };
        assert_eq!(
            test_connection(test_request(&c, "synthetic").unwrap(), flag()).unwrap(),
            "Hello!"
        );
        let request = worker.join().unwrap();
        let body: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["messages"][1]["content"], TEST_USER_TEXT);
        assert!(!request.contains("selected_text"));
    }
    #[test]
    fn bounds_body_without_content_length() {
        let response = format!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{}",
            "x".repeat(MAX_MODELS_BYTES + 1)
        );
        let (url, worker) = fixture(response);
        let err = load_models(
            models_request(Provider::OpenAIChat, &url, "").unwrap(),
            flag(),
        )
        .unwrap_err();
        assert!(err.contains("1 MiB"));
        worker.join().unwrap();
    }
    #[test]
    fn cancellation_after_dispatch_suppresses_success() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let cancel = flag();
        let worker_cancel = cancel.clone();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut buf = [0u8; 4096];
            assert!(stream.read(&mut buf).unwrap() > 0);
            worker_cancel.store(true, Ordering::Release);
            let _ = stream.write_all(reply(r#"{"data":[{"id":"valid"}]}"#).as_bytes());
        });
        assert!(
            load_models(
                models_request(Provider::OpenAIChat, &url, "").unwrap(),
                cancel
            )
            .unwrap_err()
            .contains("cancelled")
        );
        worker.join().unwrap();
    }
    #[test]
    fn stalled_response_has_finite_total_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut buf = [0u8; 4096];
            assert!(stream.read(&mut buf).unwrap() > 0);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n")
                .unwrap();
            let _ = release_rx.recv_timeout(Duration::from_secs(3));
        });
        let start = std::time::Instant::now();
        let result = load_models_with_timeout(
            models_request(Provider::OpenAIChat, &url, "").unwrap(),
            flag(),
            Duration::from_millis(100),
        );
        let _ = release_tx.send(());
        worker.join().unwrap();
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn saved_selection_and_explicit_overrides_resolve_without_environment() {
        let settings = bellobox_core::settings::Settings {
            provider_kind: "responses".into(),
            provider_endpoint: "http://127.0.0.1:1/v1".into(),
            provider_model: "saved-model".into(),
            ..Default::default()
        };
        let saved = super::super::config_from_settings(settings.clone(), None, None, None).unwrap();
        assert_eq!(saved.provider, Provider::OpenAIResponses);
        assert_eq!(saved.model, "saved-model");
        assert_eq!(saved.endpoint, settings.provider_endpoint);
        let explicit = super::super::config_from_settings(
            settings.clone(),
            Some("anthropic"),
            Some("http://127.0.0.1:2/v1"),
            Some("explicit-model"),
        )
        .unwrap();
        assert_eq!(explicit.provider, Provider::Anthropic);
        assert_eq!(explicit.endpoint, "http://127.0.0.1:2/v1");
        assert_eq!(explicit.model, "explicit-model");
        let changed =
            super::super::config_from_settings(settings, Some("anthropic"), None, None).unwrap();
        assert_eq!(changed.endpoint, "https://api.anthropic.com/v1");
    }
}
