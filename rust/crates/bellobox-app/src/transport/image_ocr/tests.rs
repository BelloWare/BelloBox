use super::fixture::{self, ResponseMode};
use super::*;
use bellobox_core::{
    ai::{Config, Provider},
    screenshot::{
        AnnotationKind, AnnotationStyle, MaskPattern, Point, Rect, RgbaColor, ScreenshotDocument,
        ScreenshotEditSession,
    },
};
use image::{Rgba, RgbaImage};
use serde_json::Value;
use std::{
    net::TcpListener,
    process::Command,
    sync::{Arc, mpsc},
    thread,
};

fn image(session: &fixture::FixtureSession) -> PreparedImage {
    PreparedImage::prepare(&session.document, &UploadOptions::default()).unwrap()
}
fn wait_request(permit: &fixture::FixturePermit) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while fixture::request_count(permit) == 0 {
        assert!(Instant::now() < deadline, "Fixture request did not arrive.");
        thread::sleep(Duration::from_millis(5));
    }
}
fn decode_base64(input: &str) -> Vec<u8> {
    // Test-only independent standard Base64 decoder avoids sharing production
    // request serialization and introduces no new application dependency.
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0;
    for byte in input.bytes().take_while(|b| *b != b'=') {
        let value = alphabet
            .iter()
            .position(|v| *v == byte)
            .expect("Invalid fixture Base64");
        accumulator = (accumulator << 6) | (value as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((accumulator >> bits) as u8);
        }
    }
    result
}
fn observed_png(value: &Value, provider: Provider) -> Vec<u8> {
    let encoded = match provider {
        Provider::OpenAIChat => value["messages"][0]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .strip_prefix("data:image/png;base64,")
            .unwrap(),
        Provider::OpenAIResponses => value["input"][0]["content"][1]["image_url"]
            .as_str()
            .unwrap()
            .strip_prefix("data:image/png;base64,")
            .unwrap(),
        Provider::Anthropic => value["messages"][0]["content"][0]["source"]["data"]
            .as_str()
            .unwrap(),
    };
    decode_base64(encoded)
}

#[test]
fn ordinary_gate_is_closed_even_for_sealed_fixture_configuration() {
    let session = fixture::start(ResponseMode::Success).unwrap();
    let prepared = image(&session);
    let cancel = AtomicBool::new(false);
    assert!(production_authority().is_err());
    assert!(
        send(
            &prepared,
            &UploadOptions::default(),
            &session.authority.lease(),
            &cancel
        )
        .is_err()
    );
    assert_eq!(fixture::request_count(&session.permit), 0);
}
#[test]
fn no_connection_before_explicit_dispatch_or_after_cancel_revoke_wrong_source_owner() {
    let session = fixture::start(ResponseMode::Success).unwrap();
    let prepared = image(&session);
    let lease = session.authority.lease();
    assert_eq!(fixture::request_count(&session.permit), 0);
    assert!(
        send_fixture(
            &prepared,
            &UploadOptions::default(),
            &lease,
            &AtomicBool::new(true),
            &session.permit
        )
        .is_err()
    );
    let wrong =
        ScreenshotDocument::from_rgba(RgbaImage::from_pixel(960, 540, Rgba([1, 2, 3, 255])))
            .unwrap();
    let wrong = PreparedImage::prepare(&wrong, &UploadOptions::default()).unwrap();
    assert!(
        send_fixture(
            &wrong,
            &UploadOptions::default(),
            &lease,
            &AtomicBool::new(false),
            &session.permit
        )
        .is_err()
    );
    let impostor = ProviderAuthority::new(
        Config {
            provider: lease.provider(),
            endpoint: lease.destination().into(),
            model: lease.model().into(),
            system_prompt: String::new(),
            max_output_tokens: 4096,
            generation_options: Default::default(),
        },
        String::new(),
    )
    .unwrap();
    assert!(
        send_fixture(
            &prepared,
            &UploadOptions::default(),
            &impostor.lease(),
            &AtomicBool::new(false),
            &session.permit
        )
        .is_err()
    );
    session.authority.revoke();
    assert!(
        send_fixture(
            &prepared,
            &UploadOptions::default(),
            &lease,
            &AtomicBool::new(false),
            &session.permit
        )
        .is_err()
    );
    assert_eq!(fixture::request_count(&session.permit), 0);
}
#[test]
fn real_loopback_builders_send_exact_approved_png_model_path_and_headers() {
    for provider in [
        Provider::OpenAIChat,
        Provider::OpenAIResponses,
        Provider::Anthropic,
    ] {
        let session = fixture::start_for(ResponseMode::Success, provider, false).unwrap();
        let mut edit = ScreenshotEditSession::new(session.document.clone());
        edit.set_crop(Some(Rect::new(0., 0., 180., 190.))).unwrap();
        edit.add_annotation(
            AnnotationKind::Blur(Rect::new(80., 100., 40., 40.)),
            AnnotationStyle::mask(RgbaColor::from_rgb8(0, 0, 0), MaskPattern::Solid),
        )
        .unwrap();
        edit.erase_stroke(vec![Point::new(95., 115.)], 8.).unwrap();
        let prepared = PreparedImage::prepare(edit.document(), &UploadOptions::default()).unwrap();
        let approved_png = prepared.png().to_vec();
        let preview = prepared.decode_preview().unwrap();
        let result = send_fixture(
            &prepared,
            &UploadOptions::default(),
            &session.authority.lease(),
            &AtomicBool::new(false),
            &session.permit,
        )
        .unwrap();
        assert!(result.plain_text.starts_with("Synthetic OCR result"));
        assert!(!result.plain_text.contains("HIDDEN_REASONING"));
        assert_eq!(fixture::request_count(&session.permit), 1);
        let request = fixture::take_observed(&session.permit).unwrap();
        let path = match provider {
            Provider::OpenAIChat => "/v1/chat/completions",
            Provider::OpenAIResponses => "/v1/responses",
            Provider::Anthropic => "/v1/messages",
        };
        assert_eq!(request.path, path);
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["model"], "fixture-vision-model");
        assert_eq!(body["stream"], false);
        let bytes = observed_png(&body, provider);
        assert!(
            bytes == approved_png,
            "Sent bytes must be the retained approval bytes."
        );
        let received = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        assert!(
            received == preview,
            "Received PNG must equal the approval preview."
        );
        assert_eq!(received.dimensions(), (180, 190));
        assert_eq!(received.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(received.get_pixel(110, 110).0, [0, 0, 0, 255]);
        assert_eq!(received.get_pixel(95, 115).0, [243, 135, 35, 255]);
        if provider == Provider::Anthropic {
            assert!(
                request
                    .headers
                    .iter()
                    .any(|(name, value)| name == "x-api-key" && value == "synthetic-fixture-key")
            );
        } else {
            assert!(
                !request
                    .headers
                    .iter()
                    .any(|(name, _)| name == "authorization")
            );
        }
    }
}
#[test]
fn response_modes_parse_plain_markdown_and_chunked_literal_results() {
    for mode in [
        ResponseMode::Success,
        ResponseMode::PlainText,
        ResponseMode::MarkdownOnly,
        ResponseMode::Chunked,
    ] {
        let session = fixture::start(mode).unwrap();
        let result = send_fixture(
            &image(&session),
            &UploadOptions::default(),
            &session.authority.lease(),
            &AtomicBool::new(false),
            &session.permit,
        )
        .unwrap();
        assert!(!result.plain_text.is_empty());
        if mode == ResponseMode::PlainText {
            assert!(result.plain_text.starts_with("[unclear]"));
            assert!(result.markdown_text.is_none());
        }
        if mode == ResponseMode::MarkdownOnly {
            assert_eq!(
                result.markdown_text.as_deref(),
                Some(result.plain_text.as_str())
            );
        }
        assert_eq!(fixture::request_count(&session.permit), 1);
    }
}
#[test]
fn error_redirect_disconnect_oversize_invalid_and_timeouts_never_retry_or_echo_body() {
    for mode in [
        ResponseMode::HttpError,
        ResponseMode::Redirect,
        ResponseMode::Disconnect,
        ResponseMode::Oversize,
        ResponseMode::OversizeChunked,
        ResponseMode::InvalidJson,
        ResponseMode::Slow,
        ResponseMode::SlowTrickle,
    ] {
        let session = fixture::start(mode).unwrap();
        let start = Instant::now();
        let error = send_fixture(
            &image(&session),
            &UploadOptions::default(),
            &session.authority.lease(),
            &AtomicBool::new(false),
            &session.permit,
        )
        .err()
        .expect("Finite error mode must fail.");
        assert!(!error.contains("PRIVATE_SYNTHETIC"));
        assert!(!error.contains("data:image"));
        if mode == ResponseMode::SlowTrickle {
            assert!(
                error.contains("timed out"),
                "Slow trickle must end at its absolute deadline."
            );
            assert!(
                fixture::read_attempts(&session.permit) >= 3,
                "The deadline must cover an active trickling body."
            );
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Finite transport deadline was not enforced."
        );
        assert_eq!(fixture::request_count(&session.permit), 1);
        assert_eq!(fixture::redirect_request_count(&session.permit), 0);
    }
}
#[test]
fn logical_cancel_during_blocking_read_waits_for_physical_return_and_cannot_publish() {
    let session = fixture::start(ResponseMode::HeldBody).unwrap();
    let prepared = image(&session);
    let lease = session.authority.lease();
    let permit = session.permit.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let (finished, done) = mpsc::channel();
    let worker = thread::spawn(move || {
        let result = send_fixture(
            &prepared,
            &UploadOptions::default(),
            &lease,
            &worker_cancel,
            &permit,
        );
        finished.send(result.is_err()).unwrap();
    });
    wait_request(&session.permit);
    let deadline = Instant::now() + Duration::from_secs(2);
    while fixture::read_attempts(&session.permit) < 2 {
        assert!(
            Instant::now() < deadline,
            "Transport did not enter its held body read."
        );
        thread::sleep(Duration::from_millis(2));
    }
    cancel.store(true, Ordering::Release);
    assert!(
        done.try_recv().is_err(),
        "Cancellation must not pretend blocked I/O has retired."
    );
    fixture::release_body(&session.permit);
    assert!(done.recv_timeout(Duration::from_secs(3)).unwrap());
    worker.join().unwrap();
    assert_eq!(fixture::request_count(&session.permit), 1);
}
#[test]
fn explicit_revocation_during_successful_read_blocks_publication() {
    let session = fixture::start(ResponseMode::HeldBody).unwrap();
    let prepared = image(&session);
    let lease = session.authority.lease();
    let permit = session.permit.clone();
    let worker = thread::spawn(move || {
        send_fixture(
            &prepared,
            &UploadOptions::default(),
            &lease,
            &AtomicBool::new(false),
            &permit,
        )
        .is_err()
    });
    wait_request(&session.permit);
    let deadline = Instant::now() + Duration::from_secs(2);
    while fixture::read_attempts(&session.permit) < 2 {
        assert!(
            Instant::now() < deadline,
            "Transport did not enter its held body read."
        );
        thread::sleep(Duration::from_millis(2));
    }
    session.authority.revoke();
    assert!(!worker.is_finished());
    fixture::release_body(&session.permit);
    assert!(worker.join().unwrap());
    assert_eq!(fixture::request_count(&session.permit), 1);
}
#[test]
fn numeric_ipv6_loopback_uses_same_sealed_transport_when_available() {
    let session = match fixture::start_for(ResponseMode::Success, Provider::OpenAIResponses, true) {
        Ok(session) => session,
        Err(_) => {
            eprintln!(
                "Numeric IPv6 loopback was unavailable; no IPv6 transport execution occurred."
            );
            return;
        }
    };
    assert!(
        session
            .authority
            .lease()
            .destination()
            .starts_with("http://[::1]:")
    );
    assert!(
        send_fixture(
            &image(&session),
            &UploadOptions::default(),
            &session.authority.lease(),
            &AtomicBool::new(false),
            &session.permit
        )
        .is_ok()
    );
    assert_eq!(fixture::request_count(&session.permit), 1);
}
#[test]
fn ambient_proxy_environment_is_ignored_in_isolated_subprocess() {
    let proxy = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    proxy.set_nonblocking(true).unwrap();
    let address = proxy.local_addr().unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "transport::image_ocr::tests::ambient_proxy_child",
            "--nocapture",
        ])
        .env("BELLOBOX_TEST_OCR_PROXY_CHILD", "1")
        .env("NO_PROXY", "")
        .env("no_proxy", "");
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env(key, format!("http://{address}"));
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "Isolated no-proxy subprocess failed."
    );
    assert!(
        matches!(proxy.accept(),Err(ref e) if e.kind()==std::io::ErrorKind::WouldBlock),
        "Ambient proxy trap received a connection."
    );
}
#[test]
fn ambient_proxy_child() {
    if std::env::var("BELLOBOX_TEST_OCR_PROXY_CHILD").as_deref() != Ok("1") {
        return;
    }
    let session = fixture::start(ResponseMode::Success).unwrap();
    assert!(
        send_fixture(
            &image(&session),
            &UploadOptions::default(),
            &session.authority.lease(),
            &AtomicBool::new(false),
            &session.permit
        )
        .is_ok()
    );
    assert_eq!(fixture::request_count(&session.permit), 1);
}

#[test]
fn held_body_read_deadline_fires_before_the_separate_total_deadline() {
    let session = fixture::start(ResponseMode::HeldBody).unwrap();
    let prepared = image(&session);
    let started = Instant::now();
    let error = send_fixture(
        &prepared,
        &UploadOptions::default(),
        &session.authority.lease(),
        &AtomicBool::new(false),
        &session.permit,
    )
    .err()
    .expect("The held body must reach its read timeout.");
    assert!(error.contains("timed out"));
    assert!(fixture::read_attempts(&session.permit) >= 2);
    assert!(
        started.elapsed() < Duration::from_millis(1_200),
        "The 500 ms read deadline must precede the 1,600 ms total deadline."
    );
    assert_eq!(fixture::request_count(&session.permit), 1);
}

// Test-only admission: a successful connect does not guarantee that a
// nonblocking listener already has an accept-ready connection on every OS.
#[cfg(unix)]
fn accept_fixture_when_ready(
    listener: &TcpListener,
    timeout: Duration,
    mut waiting: impl FnMut(),
) -> std::io::Result<std::net::TcpStream> {
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, address)) => {
                assert!(address.ip().is_loopback());
                return Ok(stream);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Fixture connection did not become accept-ready before its deadline.",
                    ));
                }
                waiting();
                thread::sleep(remaining.min(Duration::from_millis(5)));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(unix)]
#[test]
fn fixture_accept_waits_for_connection_after_observed_would_block() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (waiting, observed) = mpsc::channel();
    let (finished, result) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut waiting = Some(waiting);
        let stream = accept_fixture_when_ready(&listener, Duration::from_secs(5), || {
            if let Some(waiting) = waiting.take() {
                waiting.send(()).unwrap();
            }
        });
        finished.send(stream).unwrap();
    });
    // The connection is not created until the helper has observed not-ready.
    observed.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    let client = std::net::TcpStream::connect(address).unwrap();
    let accepted = result
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(accepted.peer_addr().unwrap(), client.local_addr().unwrap());
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn fixture_accept_without_connection_reports_explicit_bounded_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let timeout = Duration::from_millis(20);
    let started = Instant::now();
    let error = accept_fixture_when_ready(&listener, timeout, || {}).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert!(error.to_string().contains("accept-ready"));
    assert!(started.elapsed() >= timeout);
}

#[cfg(unix)]
#[test]
fn accepted_fixture_stream_normalizes_inherited_nonblocking_flags() {
    use std::os::fd::AsRawFd;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let _client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let stream = accept_fixture_when_ready(&listener, Duration::from_secs(5), || {}).unwrap();
    // Compare the kernel's canonical timeout values, which can be rounded to
    // timer ticks (this Linux host reports52ms for a requested50ms).
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let configured_timeouts = (
        stream.read_timeout().unwrap(),
        stream.write_timeout().unwrap(),
    );
    // Force Darwin's inherited state on every Unix runner, including Linux.
    stream.set_nonblocking(true).unwrap();
    let before = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFL) };
    assert!(before >= 0 && before & libc::O_NONBLOCK != 0);
    fixture::configure_sync_stream(&stream).unwrap();
    let after = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFL) };
    assert!(after >= 0);
    assert_eq!(after & libc::O_NONBLOCK, 0);
    assert_eq!(
        (
            stream.read_timeout().unwrap(),
            stream.write_timeout().unwrap()
        ),
        configured_timeouts
    );
}
