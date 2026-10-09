use super::*;
use bellobox_core::developer::http_request::{HttpRequestDraft, MAX_RESPONSE_BYTES};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex, atomic::AtomicUsize},
    thread,
};

pub(crate) struct Server {
    pub url: String,
    pub count: Arc<AtomicUsize>,
    pub requests: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
pub(crate) fn server(handler: impl Fn(&mut TcpStream, usize) + Send + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/synthetic", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (c, s, r) = (count.clone(), stop.clone(), requests.clone());
    let worker = thread::spawn(move || {
        while !s.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    let index = c.fetch_add(1, Ordering::AcqRel);
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut chunk = [0; 4096];
                    loop {
                        let n = stream.read(&mut chunk).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        request.extend_from_slice(&chunk[..n]);
                        if let Some(end) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                            let text = String::from_utf8_lossy(&request[..end]);
                            let length = text
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|v| v.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if request.len() >= end + 4 + length {
                                break;
                            }
                        }
                        assert!(request.len() < 600_000);
                    }
                    r.lock().unwrap().push(request);
                    handler(&mut stream, index);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(error) => panic!("synthetic listener failed: {error}"),
            }
        }
    });
    Server {
        url,
        count,
        stop,
        requests,
        worker: Some(worker),
    }
}
pub(crate) fn fixed(status: u16, headers: &str, body: Vec<u8>) -> Server {
    let head = format!(
        "HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
        body.len()
    );
    server(move |stream, _| {
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
    })
}
pub(crate) fn draft(server: &Server) -> HttpRequestDraft {
    HttpRequestDraft {
        url: server.url.clone(),
        ..Default::default()
    }
}
fn limits() -> Limits {
    Limits {
        connect: Duration::from_millis(150),
        read: Duration::from_millis(150),
        total: Duration::from_secs(2),
    }
}
fn run(draft: HttpRequestDraft) -> Result<HttpInspectionResult, String> {
    dispatch(draft.prepare().unwrap(), &AtomicBool::new(false), limits())
}

#[test]
fn exact_patch_headers_body_and_lossless_response() {
    let body = "{\"n\":9007199254740993,\"text\":\"雪😀\"}";
    let s = fixed(
        200,
        "X-Result: first\r\nX-Result: second\r\n",
        body.as_bytes().to_vec(),
    );
    let mut d = draft(&s);
    d.method = "PATCH".into();
    d.body = body.into();
    d.headers = "Content-Type: application/json\nX-Fixture: first\nX-Fixture: second\nContent-Length: 1\nTransfer-Encoding: chunked".into();
    let response = run(d).unwrap();
    assert!(response.body().contains("9007199254740993"));
    assert!(response.body().contains("雪😀"));
    assert!(
        response
            .headers()
            .contains("x-result: first\nx-result: second")
    );
    let requests = s.requests.lock().unwrap();
    let text = std::str::from_utf8(&requests[0]).unwrap();
    assert!(text.starts_with("PATCH /synthetic HTTP/1.1\r\n"));
    assert!(text.contains("x-fixture: first\r\nx-fixture: second\r\n"));
    assert!(text.ends_with(body));
    assert!(!text.contains("transfer-encoding:"));
    assert!(text.contains(&format!("content-length: {}", body.len())));
}
#[test]
fn redirect_and_error_statuses_are_responses_and_cookie_state_is_ephemeral() {
    let trap = fixed(200, "", b"must never be read".to_vec());
    let s = fixed(
        307,
        &format!("Location: {}\r\nSet-Cookie: synthetic=secret\r\n", trap.url),
        b"redirect body".to_vec(),
    );
    for _ in 0..2 {
        let result = run(draft(&s)).unwrap();
        assert!(result.status().starts_with("HTTP 307"));
        assert_eq!(result.body(), "redirect body");
    }
    assert_eq!(trap.count.load(Ordering::Acquire), 0);
    for request in s.requests.lock().unwrap().iter() {
        assert!(
            !String::from_utf8_lossy(request)
                .to_lowercase()
                .contains("cookie:")
        );
        assert!(
            !String::from_utf8_lossy(request)
                .to_lowercase()
                .contains("authorization:")
        );
    }
    for code in [400, 404, 418, 500, 503] {
        let s = fixed(code, "", format!("synthetic error {code}").into_bytes());
        let result = run(draft(&s)).unwrap();
        assert!(result.status().starts_with(&format!("HTTP {code}")));
        assert!(result.body().contains("synthetic error"));
    }
}
#[test]
fn cap_minus_exact_plus_and_boundary_split_utf8_use_observed_bytes() {
    for len in [
        MAX_RESPONSE_BYTES - 1,
        MAX_RESPONSE_BYTES,
        MAX_RESPONSE_BYTES + 1,
    ] {
        let s = fixed(200, "", vec![b'x'; len]);
        let result = run(draft(&s)).unwrap();
        assert_eq!(result.retained_len(), len.min(MAX_RESPONSE_BYTES));
        assert_eq!(result.truncated(), len > MAX_RESPONSE_BYTES);
    }
    let mut bytes = vec![b'x'; MAX_RESPONSE_BYTES - 1];
    bytes.extend_from_slice("😀".as_bytes());
    let s = fixed(200, "", bytes);
    let result = run(draft(&s)).unwrap();
    assert!(result.truncated());
    assert_eq!(result.body(), "Binary response (2,000,000 bytes).");
}
#[test]
fn head_empty_204_binary_and_chunked_bodies() {
    let s = server(|stream, _| {
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9000000\r\nConnection: close\r\n\r\n");
    });
    let mut d = draft(&s);
    d.method = "HEAD".into();
    let result = run(d).unwrap();
    assert_eq!(result.retained_len(), 0);
    assert!(!result.truncated());
    for status in [200, 204, 304] {
        let s = fixed(status, "", Vec::new());
        let result = run(draft(&s)).unwrap();
        assert_eq!(result.body(), "");
        assert_eq!(result.retained_len(), 0);
        assert!(!result.truncated());
    }
    let s = fixed(200, "", vec![0xff, 0xfe, 0x00]);
    assert_eq!(run(draft(&s)).unwrap().body(), "Binary response (3 bytes).");
    let s = server(|stream, _| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\none\r\n3\r\ntwo\r\n0\r\n\r\n");
    });
    assert_eq!(run(draft(&s)).unwrap().body(), "onetwo");
}
#[test]
fn lying_length_disconnect_and_no_automatic_retry() {
    let s = server(|stream, _| {
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Length: 9000000\r\nConnection: close\r\n\r\nshort",
        );
    });
    let error = run(draft(&s)).unwrap_err();
    assert!(error.contains("interrupted"));
    assert!(!error.contains("short"));
    assert_eq!(s.count.load(Ordering::Acquire), 1);
    let s = server(|_, _| {});
    assert!(run(draft(&s)).is_err());
    assert_eq!(s.count.load(Ordering::Acquire), 1);
}
#[test]
fn raw_header_bounds_and_obs_text_rendering() {
    let s = server(|stream, _| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nX-Fixture: caf\xe9\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    });
    let result = run(draft(&s)).unwrap();
    assert!(result.headers().contains("[non-UTF-8 bytes] caf\\xe9"));
    assert_eq!(result.body(), "ok");
    let s = fixed(
        200,
        &format!("X-Large: {}\r\n", "a".repeat(MAX_HEADER_BYTES)),
        b"hidden".to_vec(),
    );
    assert!(run(draft(&s)).is_err());
    let headers = (0..MAX_HEADER_COUNT + 1)
        .map(|i| format!("X-{i}: x\r\n"))
        .collect::<String>();
    let s = fixed(200, &headers, b"hidden".to_vec());
    assert!(run(draft(&s)).is_err());
}
#[test]
fn cancellation_before_send_and_during_headers_stops_without_retries() {
    let s = fixed(200, "", b"unused".to_vec());
    let result = dispatch(
        draft(&s).prepare().unwrap(),
        &AtomicBool::new(true),
        limits(),
    );
    assert_eq!(result.unwrap_err(), CANCELLED);
    assert_eq!(s.count.load(Ordering::Acquire), 0);
    let s = server(|_, _| thread::sleep(Duration::from_millis(200)));
    let request = draft(&s).prepare().unwrap();
    let signal = Arc::new(AtomicBool::new(false));
    let worker_signal = signal.clone();
    let start = Instant::now();
    let worker = thread::spawn(move || dispatch(request, &worker_signal, limits()));
    while s.count.load(Ordering::Acquire) == 0 {
        assert!(start.elapsed() < Duration::from_secs(1));
        thread::sleep(Duration::from_millis(1));
    }
    signal.store(true, Ordering::Release);
    assert_eq!(worker.join().unwrap().unwrap_err(), CANCELLED);
    assert!(start.elapsed() < Duration::from_millis(190));
    assert_eq!(s.count.load(Ordering::Acquire), 1);
}
#[test]
fn read_timeout_and_absolute_deadline_bound_a_continuous_trickle() {
    let s = server(|stream, _| {
        let _ =
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n");
        thread::sleep(Duration::from_millis(160));
    });
    let short = Limits {
        connect: Duration::from_millis(50),
        read: Duration::from_millis(50),
        total: Duration::from_millis(120),
    };
    let start = Instant::now();
    assert!(dispatch(draft(&s).prepare().unwrap(), &AtomicBool::new(false), short).is_err());
    assert!(start.elapsed() < Duration::from_millis(140));
    let s = server(|stream, _| {
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
        );
        for _ in 0..30 {
            if stream.write_all(b"1\r\nx\r\n").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(15));
        }
    });
    let start = Instant::now();
    assert!(dispatch(draft(&s).prepare().unwrap(), &AtomicBool::new(false), short).is_err());
    assert!(start.elapsed() >= Duration::from_millis(100));
    assert!(start.elapsed() < Duration::from_millis(190));
}

#[test]
fn upstream_http1_header_count_limit_is_characterized() {
    // The two framing headers in fixed() count toward Hyper's 100-field limit.
    for (total, accepted) in [(100, true), (101, false)] {
        let headers = (0..total - 2)
            .map(|i| format!("X-{i}: synthetic\r\n"))
            .collect::<String>();
        let fixture = fixed(200, &headers, b"ok".to_vec());
        assert_eq!(
            run(draft(&fixture)).is_ok(),
            accepted,
            "total wire fields: {total}"
        );
    }
}

#[test]
fn cancellation_during_body_aborts_the_live_exchange() {
    let body_started = Arc::new(AtomicBool::new(false));
    let started = body_started.clone();
    let s = server(move |stream, _| {
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nx");
        started.store(true, Ordering::Release);
        thread::sleep(Duration::from_millis(300));
    });
    let request = draft(&s).prepare().unwrap();
    let signal = Arc::new(AtomicBool::new(false));
    let worker_signal = signal.clone();
    let worker = thread::spawn(move || dispatch(request, &worker_signal, limits()));
    let start = Instant::now();
    while !body_started.load(Ordering::Acquire) {
        assert!(start.elapsed() < Duration::from_secs(1));
        thread::sleep(Duration::from_millis(1));
    }
    signal.store(true, Ordering::Release);
    assert_eq!(worker.join().unwrap().unwrap_err(), CANCELLED);
    assert!(start.elapsed() < Duration::from_millis(250));
}
#[test]
fn ambient_proxy_is_not_used_in_isolated_child_process() {
    let trap = fixed(200, "", b"proxy trap".to_vec());
    let real = fixed(200, "", b"direct synthetic".to_vec());
    let proxy = trap.url.trim_end_matches("/synthetic");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("transport::http_request::tests::ambient_proxy_child")
        .arg("--test-threads=1")
        .env("BELLOBOX_HTTP_PROXY_TEST_TARGET", &real.url)
        .env("HTTP_PROXY", proxy)
        .env("http_proxy", proxy)
        .env("HTTPS_PROXY", proxy)
        .env("https_proxy", proxy)
        .env("ALL_PROXY", proxy)
        .env("all_proxy", proxy)
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(real.count.load(Ordering::Acquire), 1);
    assert_eq!(trap.count.load(Ordering::Acquire), 0);
}
#[test]
fn ambient_proxy_child() {
    let Ok(url) = std::env::var("BELLOBOX_HTTP_PROXY_TEST_TARGET") else {
        return;
    };
    assert!(url.starts_with("http://127.0.0.1:"));
    let result = run(HttpRequestDraft {
        url,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.body(), "direct synthetic");
}
