//! QR worker payloads. Call only on the background executor after cheap validation.
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct Preview {
    pub png: Vec<u8>,
}

pub(crate) fn validate(input: &str) -> Result<(), String> {
    if input.len() > bellobox_core::qr::MAX_QR_BYTES || input.trim().is_empty() {
        Err("QR content must be 1–2,000 UTF-8 bytes.".into())
    } else {
        Ok(())
    }
}

/// Cancelled queued requests do no encoding. An already-running encode can finish,
/// but its result still needs the window's SessionJobs token check before publication.
pub(crate) fn generate(input: &str, cancelled: &AtomicBool) -> Option<Result<Preview, String>> {
    if cancelled.load(Ordering::Relaxed) {
        return None;
    }
    let result = (|| {
        validate(input)?;
        let png = bellobox_core::qr::png(input)?;
        if cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        Ok(Some(Preview { png }))
    })();
    match result {
        Ok(preview) => preview.map(Ok),
        Err(error) => Some(Err(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{JobToken, SessionJobs};

    // Deterministically deliver worker completions in arbitrary order using the
    // exact SessionJobs acceptance gate used by both UI completion handlers.
    fn publish(
        jobs: &SessionJobs,
        token: JobToken,
        result: Result<&'static str, &'static str>,
        shown: &mut Option<Result<&'static str, &'static str>>,
    ) {
        if jobs.accepts(token) {
            *shown = Some(result);
        }
    }

    #[test]
    fn reordered_success_and_error_never_replace_newer_generation() {
        for (new, old) in [
            (Ok("B"), Ok("A")),
            (Err("B error"), Ok("A")),
            (Ok("B"), Err("A error")),
        ] {
            let mut jobs = SessionJobs::default();
            let a = jobs.begin();
            let b = jobs.begin();
            let mut shown = None;
            publish(&jobs, b, new, &mut shown);
            publish(&jobs, a, old, &mut shown);
            assert_eq!(shown, Some(new));
        }
    }

    #[test]
    fn empty_blank_and_oversize_edits_invalidate_pending_copy() {
        for input in [
            "".to_string(),
            " \n\t".into(),
            "x".repeat(2001),
            "界".repeat(667),
        ] {
            let mut jobs = SessionJobs::default();
            let pending = jobs.begin();
            let cancelled = jobs.cancellation();
            jobs.begin(); // Every edit begins before validation/early returns.
            let mut copyable = None; // UI clears the prior image at edit time.
            assert!(validate(&input).is_err());
            publish(&jobs, pending, Ok("old image"), &mut copyable);
            assert!(copyable.is_none());
            assert!(generate("old", &cancelled).is_none());
        }
    }

    #[test]
    fn rapid_edits_only_latest_queued_work_encodes() {
        let mut jobs = SessionJobs::default();
        let requests: Vec<_> = (0..200)
            .map(|_| {
                let token = jobs.begin();
                (token, jobs.cancellation())
            })
            .collect();
        for (token, flag) in &requests[..199] {
            assert!(!jobs.accepts(*token));
            assert!(generate("queued", flag).is_none());
        }
        assert!(jobs.accepts(requests[199].0));
        assert!(generate("latest", &requests[199].1).unwrap().is_ok());
    }

    #[test]
    fn cancel_close_and_reopen_reject_old_window_results() {
        let (old, flag) = {
            let mut jobs = SessionJobs::default();
            let old = jobs.begin();
            let flag = jobs.cancellation();
            jobs.cancel();
            assert!(!jobs.accepts(old));
            (jobs.begin(), {
                assert!(flag.load(Ordering::Relaxed));
                jobs.cancellation()
            })
        };
        assert!(generate("closed", &flag).is_none());
        let mut reopened = SessionJobs::default();
        reopened.begin();
        assert!(!reopened.accepts(old));
    }

    #[test]
    fn save_snapshot_stays_click_time_while_status_is_generation_guarded() {
        let mut previews = SessionJobs::default();
        let mut saves = SessionJobs::default();
        let preview = previews.begin();
        let mut input = "click-time text".to_string();
        let snapshot = input.clone();
        let first_save = saves.begin();
        // Save during pending preview does not invalidate its current image.
        assert!(previews.accepts(preview));
        input.clear();
        input.push_str("new edit");
        previews.begin();
        saves.cancel();
        assert!(!saves.accepts(first_save));
        assert_eq!(snapshot, "click-time text");
        assert_ne!(snapshot, input);
        let second_save = saves.begin();
        assert!(saves.accepts(second_save));
        assert!(!saves.accepts(first_save));
        assert_ne!(
            bellobox_core::qr::png(&snapshot).unwrap(),
            bellobox_core::qr::png(&input).unwrap()
        );
    }

    #[test]
    fn worker_output_matches_unchanged_core_exactly() {
        let cancelled = AtomicBool::new(false);
        for input in [
            "https://belloware.com".to_string(),
            "界🙂".repeat(50),
            "a".repeat(2000),
        ] {
            let preview = generate(&input, &cancelled).unwrap().unwrap();
            assert_eq!(preview.png, bellobox_core::qr::png(&input).unwrap());
        }
    }
}
