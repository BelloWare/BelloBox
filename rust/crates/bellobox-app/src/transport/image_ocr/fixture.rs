//! Sealed supplied-image facility. It can bind only its own ephemeral numeric
//! loopback listener, generate its own base image and return finite responses.
//! There is no arbitrary URL, path, image, key, response or global gate override.
use super::Limits;
use bellobox_core::{
    ai::{Config, Provider},
    screenshot::{
        ScreenshotDocument,
        ai_ocr::{
            DocumentBinding, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, PreparedImage,
            ProviderAuthority, ProviderLease,
        },
    },
};
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseMode {
    Success,
    PlainText,
    MarkdownOnly,
    HttpError,
    Oversize,
    Chunked,
    Slow,
    Disconnect,
    Redirect,
    DelayedSuccess,
    InvalidJson,
    OversizeChunked,
    SlowTrickle,
    HeldBody,
}
impl ResponseMode {
    pub const ALL: [Self; 14] = [
        Self::Success,
        Self::PlainText,
        Self::MarkdownOnly,
        Self::HttpError,
        Self::Oversize,
        Self::Chunked,
        Self::Slow,
        Self::Disconnect,
        Self::Redirect,
        Self::DelayedSuccess,
        Self::InvalidJson,
        Self::OversizeChunked,
        Self::SlowTrickle,
        Self::HeldBody,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::PlainText => "Plain text",
            Self::MarkdownOnly => "Markdown only",
            Self::HttpError => "HTTP error",
            Self::Oversize => "Oversize response",
            Self::Chunked => "Chunked response",
            Self::Slow => "Read timeout",
            Self::Disconnect => "Disconnect",
            Self::Redirect => "Redirect trap",
            Self::DelayedSuccess => "Delayed success",
            Self::InvalidJson => "Invalid JSON",
            Self::OversizeChunked => "Oversize chunked",
            Self::SlowTrickle => "Total timeout",
            Self::HeldBody => "Delayed body",
        }
    }
}
struct State {
    mode: Mutex<ResponseMode>,
    connections: AtomicUsize,
    redirect_connections: AtomicUsize,
    stop: AtomicBool,
    #[cfg(test)]
    observed: Mutex<Option<ObservedRequest>>,
    #[cfg(test)]
    read_attempts: Arc<AtomicUsize>,
    #[cfg(test)]
    release_body: AtomicBool,
}
struct PermitInner {
    state: Arc<State>,
    binding: DocumentBinding,
    authority: ProviderLease,
}
impl Drop for PermitInner {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
    }
}
#[derive(Clone)]
pub struct FixturePermit(Arc<PermitInner>);
pub struct FixtureSession {
    pub document: ScreenshotDocument,
    pub authority: ProviderAuthority,
    pub permit: FixturePermit,
}
impl FixturePermit {
    pub fn check_document(&self, document: &ScreenshotDocument) -> bool {
        self.0.binding.matches_document(document)
    }
    pub(super) fn validate(
        &self,
        image: &PreparedImage,
        lease: &ProviderLease,
    ) -> Result<(), String> {
        if !lease.is_valid() {
            return Err(super::REVOKED.into());
        }
        if !self.0.binding.matches_prepared(image)
            || !self.0.authority.same_authority(lease)
            || self.0.state.stop.load(Ordering::Acquire)
        {
            return Err("AI OCR fixture authority does not match its generated image and approved provider.".into());
        }
        Ok(())
    }
    pub(super) fn limits(&self) -> Limits {
        // Test-owned timing shortens failures while exercising the exact client.
        // DelayedSuccess is a finite GUI cancellation/retirement fixture.
        let mode = self
            .0
            .state
            .mode
            .lock()
            .map(|v| *v)
            .unwrap_or(ResponseMode::Disconnect);
        if mode == ResponseMode::DelayedSuccess {
            Limits {
                connect: Duration::from_secs(1),
                read: Duration::from_secs(12),
                total: Duration::from_secs(20),
                #[cfg(test)]
                read_attempts: Some(self.0.state.read_attempts.clone()),
            }
        } else {
            Limits {
                connect: Duration::from_millis(500),
                read: Duration::from_millis(500),
                total: Duration::from_millis(1_600),
                #[cfg(test)]
                read_attempts: Some(self.0.state.read_attempts.clone()),
            }
        }
    }
}
pub fn request_count(permit: &FixturePermit) -> usize {
    permit.0.state.connections.load(Ordering::Acquire)
}
pub fn redirect_request_count(permit: &FixturePermit) -> usize {
    permit.0.state.redirect_connections.load(Ordering::Acquire)
}
pub fn set_response_mode(permit: &FixturePermit, mode: ResponseMode) {
    if let Ok(mut current) = permit.0.state.mode.lock() {
        *current = mode;
    }
}
pub fn start(mode: ResponseMode) -> Result<FixtureSession, String> {
    start_internal(mode, Provider::OpenAIChat, false)
}
#[cfg(test)]
pub(crate) fn start_for(
    mode: ResponseMode,
    provider: Provider,
    ipv6: bool,
) -> Result<FixtureSession, String> {
    start_internal(mode, provider, ipv6)
}
fn start_internal(
    mode: ResponseMode,
    provider: Provider,
    ipv6: bool,
) -> Result<FixtureSession, String> {
    let ip = if ipv6 {
        IpAddr::V6(Ipv6Addr::LOCALHOST)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };
    let listener = TcpListener::bind(SocketAddr::new(ip, 0))
        .map_err(|_| "Could not start the local generated-image OCR fixture.")?;
    let address = listener
        .local_addr()
        .map_err(|_| "Could not resolve the local OCR fixture.")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "Could not initialize the local OCR fixture.")?;
    let redirect = TcpListener::bind(SocketAddr::new(ip, 0))
        .map_err(|_| "Could not start the local redirect trap.")?;
    let redirect_address = redirect
        .local_addr()
        .map_err(|_| "Could not resolve the redirect trap.")?;
    redirect
        .set_nonblocking(true)
        .map_err(|_| "Could not initialize the redirect trap.")?;
    let document = generated_document()?;
    let authority = ProviderAuthority::new(
        Config {
            provider,
            endpoint: format!("http://{address}/v1"),
            model: "fixture-vision-model".into(),
            system_prompt: "Never used by OCR".into(),
            max_output_tokens: 4_096,
        },
        if provider == Provider::Anthropic {
            "synthetic-fixture-key".into()
        } else {
            String::new()
        },
    )?;
    let state = Arc::new(State {
        mode: Mutex::new(mode),
        connections: AtomicUsize::new(0),
        redirect_connections: AtomicUsize::new(0),
        stop: AtomicBool::new(false),
        #[cfg(test)]
        observed: Mutex::new(None),
        #[cfg(test)]
        read_attempts: Arc::new(AtomicUsize::new(0)),
        #[cfg(test)]
        release_body: AtomicBool::new(false),
    });
    let permit = FixturePermit(Arc::new(PermitInner {
        state: state.clone(),
        binding: DocumentBinding::new(&document),
        authority: authority.lease(),
    }));
    let server = state.clone();
    thread::Builder::new()
        .name("ocr-loopback-fixture".into())
        .spawn(move || {
            while !server.stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        server.connections.fetch_add(1, Ordering::AcqRel);
                        serve(stream, &server, provider, redirect_address);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(|_| "Could not launch the local OCR fixture.")?;
    thread::Builder::new().name("ocr-redirect-trap".into()).spawn(move || {
        while !state.stop.load(Ordering::Acquire) {
            match redirect.accept() {
                Ok((mut stream, _)) => {
                    state.redirect_connections.fetch_add(1, Ordering::AcqRel);
                    if configure_sync_stream(&stream).is_err() { continue; }
                    let _ = stream.write_all(b"HTTP/1.1 500 Rejected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(5)),
                Err(_) => break,
            }
        }
    }).map_err(|_| "Could not launch the local redirect trap.")?;
    Ok(FixtureSession {
        document,
        authority,
        permit,
    })
}
fn generated_document() -> Result<ScreenshotDocument, String> {
    let mut image = RgbaImage::from_pixel(960, 540, Rgba([245, 245, 245, 255]));
    // Fixed, distinctive supplied pixels. The upper-left transparent patch
    // carries synthetic hidden RGB to exercise canonicalization at the real UI.
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        if x < 40 && y < 40 {
            *pixel = Rgba([231, 17, 199, 0]);
        } else if (80..880).contains(&x) && (100..180).contains(&y) {
            *pixel = Rgba([243, 135, 35, 255]);
        } else if (80..550).contains(&x) && (240..310).contains(&y) {
            *pixel = Rgba([39, 101, 188, 255]);
        } else if (80..880).contains(&x) && (390..450).contains(&y) {
            *pixel = Rgba([40, 50, 65, 255]);
        }
    }
    ScreenshotDocument::from_rgba(image)
}
pub(crate) struct ObservedRequest {
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
#[cfg(test)]
pub(crate) fn release_body(permit: &FixturePermit) {
    permit.0.state.release_body.store(true, Ordering::Release);
}
#[cfg(test)]
pub(crate) fn read_attempts(permit: &FixturePermit) -> usize {
    permit.0.state.read_attempts.load(Ordering::Acquire)
}
#[cfg(test)]
pub(crate) fn take_observed(permit: &FixturePermit) -> Option<ObservedRequest> {
    permit.0.state.observed.lock().ok()?.take()
}
pub(super) fn configure_sync_stream(stream: &TcpStream) -> std::io::Result<()> {
    // Darwin accept inherits O_NONBLOCK; Linux accept does not. The fixture's
    // synchronous parser/writer deliberately uses bounded blocking timeouts.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))
}
fn read_request(stream: &mut TcpStream, state: &State) -> Option<ObservedRequest> {
    configure_sync_stream(stream).ok()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8_192];
    let end = loop {
        if state.stop.load(Ordering::Acquire) || Instant::now() >= deadline || bytes.len() > 16_384
        {
            return None;
        }
        if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
            break end + 4;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return None,
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return None,
        }
    };
    if end > 16_384 {
        return None;
    }
    let header = std::str::from_utf8(&bytes[..end]).ok()?;
    let mut lines = header.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    if first.next()? != "POST" {
        return None;
    }
    let path = first.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|v| v.split_once(':'))
        .map(|(a, b)| (a.trim().to_ascii_lowercase(), b.trim().to_owned()))
        .collect();
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")?
        .1
        .parse::<usize>()
        .ok()?;
    if length > MAX_REQUEST_BYTES {
        return None;
    }
    let mut body = bytes.split_off(end);
    while body.len() < length {
        if state.stop.load(Ordering::Acquire) || Instant::now() >= deadline {
            return None;
        }
        let limit = buffer.len().min(length - body.len());
        match stream.read(&mut buffer[..limit]) {
            Ok(0) => return None,
            Ok(n) => body.extend_from_slice(&buffer[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return None,
        }
    }
    if body.len() != length {
        return None;
    }
    Some(ObservedRequest {
        path,
        headers,
        body,
    })
}
fn sleep_while_live(state: &State, delay: Duration) -> bool {
    let deadline = Instant::now() + delay;
    while Instant::now() < deadline {
        if state.stop.load(Ordering::Acquire) {
            return false;
        }
        thread::sleep(Duration::from_millis(5));
    }
    true
}
fn serve(mut stream: TcpStream, state: &State, provider: Provider, redirect: SocketAddr) {
    let Some(request) = read_request(&mut stream, state) else {
        return;
    };
    let expected = match provider {
        Provider::OpenAIChat => "/v1/chat/completions",
        Provider::OpenAIResponses => "/v1/responses",
        Provider::Anthropic => "/v1/messages",
    };
    if request.path != expected {
        return;
    }
    #[cfg(test)]
    if let Ok(mut observed) = state.observed.lock() {
        *observed = Some(request);
    }
    #[cfg(not(test))]
    drop((request.path, request.headers, request.body));
    let mode = state
        .mode
        .lock()
        .map(|v| *v)
        .unwrap_or(ResponseMode::Disconnect);
    if mode == ResponseMode::Disconnect {
        return;
    }
    if mode == ResponseMode::Slow && !sleep_while_live(state, Duration::from_secs(2)) {
        return;
    }
    if mode == ResponseMode::DelayedSuccess && !sleep_while_live(state, Duration::from_secs(8)) {
        return;
    }
    if mode == ResponseMode::Redirect {
        let _ = write!(
            stream,
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{redirect}/trap\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        return;
    }
    if mode == ResponseMode::HttpError {
        let _ = stream.write_all(b"HTTP/1.1 429 Rejected\r\nContent-Length: 29\r\nConnection: close\r\n\r\nPRIVATE_SYNTHETIC_ERROR_MARKER");
        return;
    }
    if mode == ResponseMode::Oversize {
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_RESPONSE_BYTES + 1
        );
        return;
    }
    let text = match mode {
        ResponseMode::PlainText => "[unclear] Synthetic OCR text".into(),
        ResponseMode::MarkdownOnly => json!({"markdownText":"| Item | Value |\n| --- | --- |\n| Synthetic | 42 |","warnings":[]}).to_string(),
        _ => json!({"plainText":"Synthetic OCR result\nGenerated image only","markdownText":"# Synthetic OCR result\nGenerated image only","warnings":["Synthetic local response; no external provider was contacted."]}).to_string(),
    };
    let body = if mode == ResponseMode::InvalidJson {
        b"PRIVATE_SYNTHETIC_INVALID_BODY".to_vec()
    } else {
        match provider {
            Provider::OpenAIChat => json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}),
            Provider::OpenAIResponses => json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":text}]}]}),
            Provider::Anthropic => json!({"content":[{"type":"thinking","thinking":"SYNTHETIC_HIDDEN_REASONING"},{"type":"text","text":text}],"stop_reason":"end_turn"}),
        }.to_string().into_bytes()
    };
    if mode == ResponseMode::HeldBody {
        if write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
            .and_then(|_| stream.write_all(&body[..1])).is_err() { return; }
        #[cfg(test)]
        {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !state.release_body.load(Ordering::Acquire) {
                if Instant::now() >= deadline || !sleep_while_live(state, Duration::from_millis(2))
                {
                    return;
                }
            }
        }
        #[cfg(not(test))]
        if !sleep_while_live(state, Duration::from_millis(300)) {
            return;
        }
        let _ = stream.write_all(&body[1..]);
        return;
    }
    if matches!(
        mode,
        ResponseMode::Chunked | ResponseMode::OversizeChunked | ResponseMode::SlowTrickle
    ) {
        if stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .is_err()
        {
            return;
        }
        if mode == ResponseMode::OversizeChunked {
            let chunk = [b' '; 8_192];
            for _ in 0..=(MAX_RESPONSE_BYTES / chunk.len()) {
                if write!(stream, "{:x}\r\n", chunk.len())
                    .and_then(|_| stream.write_all(&chunk))
                    .and_then(|_| stream.write_all(b"\r\n"))
                    .is_err()
                {
                    return;
                }
            }
        } else if mode == ResponseMode::SlowTrickle {
            // Remain live beyond the total deadline: malformed end-of-stream
            // must not masquerade as proof that the timeout fired.
            for _ in 0..100 {
                if stream.write_all(b"1\r\n \r\n").is_err()
                    || !sleep_while_live(state, Duration::from_millis(100))
                {
                    return;
                }
            }
        } else {
            for chunk in body.chunks(17) {
                if write!(stream, "{:x}\r\n", chunk.len())
                    .and_then(|_| stream.write_all(chunk))
                    .and_then(|_| stream.write_all(b"\r\n"))
                    .is_err()
                {
                    return;
                }
            }
        }
        let _ = stream.write_all(b"0\r\n\r\n");
    } else {
        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).and_then(|_| stream.write_all(&body));
    }
}
