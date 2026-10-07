//! Dedicated bounded non-streaming image transport. The ordinary admission gate
//! is checked before settings, environment credentials, clients, DNS or sockets.
//! Only sealed, generated-image numeric-loopback fixtures can exercise dispatch.
#[cfg(any(test, debug_assertions))]
pub mod fixture;
#[cfg(test)]
mod tests;

use bellobox_core::screenshot::ai_ocr::{
    ImageRequest, MAX_RESPONSE_BYTES, OcrResult, PreparedImage, ProviderAuthority, ProviderLease,
    UploadOptions, build_request, parse_response,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const PRODUCTION_AI_OCR_ENABLED: bool = false;
const UNAVAILABLE: &str = "AI OCR upload is unavailable until provider authority and native runtime validation are complete. Local OCR remains available.";
const CANCELLED: &str = "AI OCR request cancelled.";
const REVOKED: &str = "AI OCR provider authority was revoked.";

pub fn production_authority() -> Result<ProviderAuthority, String> {
    if !PRODUCTION_AI_OCR_ENABLED {
        return Err(UNAVAILABLE.into());
    }
    // No environment-key comparison can implement revocation. A future real
    // authority provider must have an explicit revoke/disconnect signal.
    Err(UNAVAILABLE.into())
}
pub fn send(
    image: &PreparedImage,
    options: &UploadOptions,
    lease: &ProviderLease,
    cancel: &AtomicBool,
) -> Result<OcrResult, String> {
    if !PRODUCTION_AI_OCR_ENABLED {
        return Err(UNAVAILABLE.into());
    }
    dispatch(image, options, lease, cancel, Limits::default())
}
#[cfg(any(test, debug_assertions))]
pub fn send_fixture(
    image: &PreparedImage,
    options: &UploadOptions,
    lease: &ProviderLease,
    cancel: &AtomicBool,
    permit: &fixture::FixturePermit,
) -> Result<OcrResult, String> {
    permit.validate(image, lease)?;
    let result = dispatch(image, options, lease, cancel, permit.limits())?;
    // Revocation can prevent publication after dispatch, but cannot retract
    // bytes already sent. The editor independently rechecks its own generation.
    permit.validate(image, lease)?;
    Ok(result)
}
#[derive(Clone)]
struct Limits {
    connect: Duration,
    read: Duration,
    total: Duration,
    #[cfg(test)]
    read_attempts: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(5),
            read: Duration::from_secs(10),
            total: Duration::from_secs(45),
            #[cfg(test)]
            read_attempts: None,
        }
    }
}
fn current(lease: &ProviderLease, cancel: &AtomicBool, deadline: Instant) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }
    if !lease.is_valid() {
        return Err(REVOKED.into());
    }
    if Instant::now() >= deadline {
        return Err("AI OCR request timed out.".into());
    }
    Ok(())
}
fn dispatch(
    image: &PreparedImage,
    options: &UploadOptions,
    lease: &ProviderLease,
    cancel: &AtomicBool,
    limits: Limits,
) -> Result<OcrResult, String> {
    let deadline = Instant::now() + limits.total;
    current(lease, cancel, deadline)?;
    let request = build_request(image, options, lease)?;
    current(lease, cancel, deadline)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Could not initialize the AI OCR worker runtime.")?;
    current(lease, cancel, deadline)?;
    // The entire response is polled inside its owned runtime. In particular,
    // reqwest's lazy read-timeout timer must never be polled by blocking::Read
    // outside a Tokio reactor. No detached request task can outlive this owner.
    let exchanged = runtime.block_on(async {
        tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            exchange(request, lease, cancel, deadline, limits),
        )
        .await
        .map_err(|_| "AI OCR request timed out.".to_owned())?
    });
    // Runtime/client disposal precedes physical worker retirement. A platform
    // resolver that cannot be interrupted may retain this worker after timeout;
    // cancellation never pretends that physical ownership has already drained.
    drop(runtime);
    let bytes = exchanged?;
    current(lease, cancel, deadline)?;
    let result = parse_response(lease.provider(), &bytes)?;
    current(lease, cancel, deadline)?;
    Ok(result)
}
async fn exchange(
    request: ImageRequest,
    lease: &ProviderLease,
    cancel: &AtomicBool,
    deadline: Instant,
    limits: Limits,
) -> Result<Vec<u8>, String> {
    current(lease, cancel, deadline)?;
    // Separate bounded connect/read timeouts plus an absolute whole-body
    // deadline prevent a slow trickle from resetting a supposed total limit.
    let remaining = deadline.saturating_duration_since(Instant::now());
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(limits.connect.min(remaining))
        .read_timeout(limits.read.min(remaining))
        .timeout(remaining)
        .connection_verbose(false)
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|_| "Could not initialize AI OCR transport.")?;
    let (url, headers, body) = request.into_parts();
    // Header conversion precedes network submission and cannot echo values.
    let mut values = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| "Invalid AI OCR request header.")?;
        let mut value = reqwest::header::HeaderValue::from_str(&value)
            .map_err(|_| "Invalid AI OCR request header.")?;
        value.set_sensitive(true);
        values.insert(name, value);
    }
    current(lease, cancel, deadline)?;
    // Recompute the remaining absolute budget after client/header preparation.
    // This request timeout is retained by the inner async request and therefore
    // covers connect, headers and the entire response body, not only one read.
    let mut response = client
        .post(url)
        .headers(values)
        .body(body)
        .timeout(deadline.saturating_duration_since(Instant::now()))
        .send()
        .await
        .map_err(|_| "AI OCR provider connection failed or timed out.")?;
    current(lease, cancel, deadline)?;
    if !response.status().is_success() {
        return Err(format!(
            "AI OCR provider returned HTTP {}. No redirect was followed.",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|v| v > MAX_RESPONSE_BYTES as u64)
    {
        return Err("AI OCR response exceeds 2 MiB.".into());
    }
    let mut bytes = Vec::new();
    loop {
        current(lease, cancel, deadline)?;
        #[cfg(test)]
        if let Some(attempts) = &limits.read_attempts {
            attempts.fetch_add(1, Ordering::AcqRel);
        }
        let chunk = response.chunk().await;
        current(lease, cancel, deadline)?;
        let Some(chunk) = chunk.map_err(|_| "AI OCR response was interrupted or timed out.")?
        else {
            break;
        };
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
            return Err("AI OCR response exceeds 2 MiB.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    // Response/client/body destructors finish inside this owned runtime.
    // Cancellation never detaches the request. Editor admission remains held
    // until runtime/worker return and prepared-output disposal finish.
    Ok(bytes)
}
