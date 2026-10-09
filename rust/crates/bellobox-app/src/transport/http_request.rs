//! Explicit HTTP inspection transport. Every exchange owns a fresh, ephemeral
//! client/runtime; no provider configuration, shell, proxy or credential lookup.
use bellobox_core::developer::http_request::{
    HttpInspectionResult, MAX_HEADER_BYTES, MAX_HEADER_COUNT, PreparedHttpRequest,
    ResponseBodyPreview, render_response,
};
use std::{
    future::{Future, poll_fn},
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
    time::{Duration, Instant},
};

const CANCELLED: &str = "Request cancelled.";
const TIMEOUT: &str = "Request timed out.";
#[derive(Clone, Copy)]
struct Limits {
    connect: Duration,
    read: Duration,
    total: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(30),
            read: Duration::from_secs(30),
            total: Duration::from_secs(60),
        }
    }
}

pub(crate) fn send(
    request: PreparedHttpRequest,
    cancel: &AtomicBool,
) -> Result<HttpInspectionResult, String> {
    dispatch(request, cancel, Limits::default())
}
fn current(cancel: &AtomicBool, deadline: Instant) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err(CANCELLED.into())
    } else if Instant::now() >= deadline {
        Err(TIMEOUT.into())
    } else {
        Ok(())
    }
}
fn dispatch(
    request: PreparedHttpRequest,
    cancel: &AtomicBool,
    limits: Limits,
) -> Result<HttpInspectionResult, String> {
    let start = Instant::now();
    let deadline = start + limits.total;
    current(cancel, deadline)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Could not initialize the HTTP worker runtime.")?;
    // Requests and lazy body timers are only polled inside their owned reactor.
    // Dropping the async exchange aborts its sockets before this worker returns.
    let result = runtime.block_on(async {
        tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            exchange(request, cancel, start, deadline, limits),
        )
        .await
        .map_err(|_| TIMEOUT.to_owned())?
    });
    // A platform resolver may finish later than cancellation; physical admission
    // remains occupied through runtime disposal. No detached request is spawned.
    drop(runtime);
    current(cancel, deadline)?;
    result
}
async fn cancellable<F: Future>(future: F, cancel: &AtomicBool) -> Result<F::Output, String> {
    let mut future = Box::pin(future);
    let mut check = Box::pin(tokio::time::sleep(Duration::from_millis(20)));
    poll_fn(|cx| {
        if cancel.load(Ordering::Acquire) {
            return Poll::Ready(Err(CANCELLED.into()));
        }
        if let Poll::Ready(result) = future.as_mut().poll(cx) {
            return Poll::Ready(Ok(result));
        }
        if check.as_mut().poll(cx).is_ready() {
            check
                .as_mut()
                .reset(tokio::time::Instant::now() + Duration::from_millis(20));
            // Register the next wake after resetting the completed timer.
            let _ = check.as_mut().poll(cx);
        }
        Poll::Pending
    })
    .await
}
async fn exchange(
    request: PreparedHttpRequest,
    cancel: &AtomicBool,
    start: Instant,
    deadline: Instant,
    limits: Limits,
) -> Result<HttpInspectionResult, String> {
    current(cancel, deadline)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .referer(false)
        .connect_timeout(limits.connect.min(remaining))
        .read_timeout(limits.read.min(remaining))
        .timeout(remaining)
        .pool_max_idle_per_host(0)
        .connection_verbose(false)
        .build()
        .map_err(|_| "Could not initialize HTTP transport.")?;
    let method = reqwest::Method::from_bytes(request.method().as_bytes())
        .map_err(|_| "Invalid HTTP method.")?;
    let mut headers = reqwest::header::HeaderMap::new();
    for (name, value) in request.headers() {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| "Invalid request header name.")?;
        let mut value = reqwest::header::HeaderValue::from_bytes(value.as_bytes())
            .map_err(|_| "Invalid request header value.")?;
        value.set_sensitive(true);
        headers.append(name, value);
    }
    current(cancel, deadline)?;
    let builder = client
        .request(method, request.url())
        .headers(headers)
        .body(request.body().to_vec())
        .timeout(deadline.saturating_duration_since(Instant::now()));
    let mut response = cancellable(builder.send(), cancel)
        .await?
        .map_err(|_| "HTTP connection failed or timed out. Check the URL and network.")?;
    current(cancel, deadline)?;
    // 3xx, 4xx and 5xx are inspectable responses, never transport errors.
    let status = response.status().as_u16();
    let raw_headers = response.headers();
    if raw_headers.len() > MAX_HEADER_COUNT {
        return Err("Response exceeds 256 headers.".into());
    }
    raw_headers.iter().try_fold(0usize, |sum, (name, value)| {
        let total = sum
            .saturating_add(name.as_str().len())
            .saturating_add(value.as_bytes().len())
            .saturating_add(3);
        (total <= MAX_HEADER_BYTES)
            .then_some(total)
            .ok_or("Response headers exceed 64,000 UTF-8 bytes.")
    })?;
    let headers = raw_headers
        .iter()
        .map(|(name, value)| {
            // HTTP/1 obs-text is not necessarily UTF-8. Escape opaque bytes
            // explicitly rather than losing the inspectable status/body or
            // pretending to know the server's text encoding. Core also bounds
            // this expanded display representation.
            let value = std::str::from_utf8(value.as_bytes())
                .map(str::to_owned)
                .unwrap_or_else(|_| {
                    format!("[non-UTF-8 bytes] {}", value.as_bytes().escape_ascii())
                });
            (name.to_string(), value)
        })
        .collect::<Vec<_>>();
    let mut preview = ResponseBodyPreview::new();
    loop {
        current(cancel, deadline)?;
        let chunk = cancellable(response.chunk(), cancel).await?;
        current(cancel, deadline)?;
        let Some(chunk) = chunk.map_err(|_| "HTTP response was interrupted or timed out.")? else {
            break;
        };
        // Content-Length is only metadata. Truncation requires an observed byte
        // beyond the cap, including HEAD/empty/lying-length responses.
        if preview.push(&chunk) {
            break;
        }
    }
    current(cancel, deadline)?;
    render_response(status, start.elapsed(), &headers, &preview)
}
#[cfg(test)]
pub(crate) mod tests;
