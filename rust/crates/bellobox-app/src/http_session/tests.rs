use super::*;
use crate::transport::http_request::tests::{draft, fixed, server};
use gpui::{AppContext, TestAppContext};
use std::{io::Write, time::Duration};
fn tick(cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(11));
    cx.run_until_parked();
}
#[gpui::test]
fn edits_are_inert_and_explicit_send_is_once_only(cx: &mut TestAppContext) {
    let fixture = fixed(404, "", b"synthetic response".to_vec());
    let session = cx.new(HttpSession::new);
    for _ in 0..3 {
        session.update(cx, |s, cx| s.set_draft(draft(&fixture), cx));
    }
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 0);
    session.update(cx, |s, cx| {
        s.send(cx);
        s.send(cx);
        assert!(s.busy);
        assert!(s.running);
    });
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 1);
    session.update(cx, |s, _| {
        assert!(s.can_copy());
        assert!(s.can_send());
        assert!(!s.running);
        assert_eq!(s.result.as_ref().unwrap().body(), "synthetic response");
    });
}
#[gpui::test]
fn cancel_edit_retry_retains_physical_ownership_and_fences_stale_result(cx: &mut TestAppContext) {
    let fixture = fixed(200, "", b"synthetic old".to_vec());
    let session = cx.new(HttpSession::new);
    let held = session.update(cx, |s, cx| {
        s.publication_held.store(true, Ordering::Release);
        s.set_draft(draft(&fixture), cx);
        s.send(cx);
        s.publication_held.clone()
    });
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 1);
    session.update(cx, |s, cx| {
        s.cancel(cx);
        assert!(s.running);
        assert!(!s.can_copy());
        assert!(!s.can_send());
        let mut edited = draft(&fixture);
        edited.method = "PATCH".into();
        edited.body = "new body".into();
        s.set_draft(edited, cx);
        s.send(cx);
        assert!(s.running);
        assert!(!s.busy);
    });
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 1);
    held.store(false, Ordering::Release);
    tick(cx);
    session.update(cx, |s, cx| {
        assert!(!s.running);
        assert!(s.result.is_none());
        assert!(s.can_send());
        s.send(cx);
    });
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 2);
    session.update(cx, |s, _| assert!(s.can_copy()));
    assert!(String::from_utf8_lossy(&fixture.requests.lock().unwrap()[1]).ends_with("new body"));
}
#[gpui::test]
fn retained_closed_and_quit_sessions_reject_completion_and_dispose(cx: &mut TestAppContext) {
    let fixture = fixed(200, "", b"synthetic private response".to_vec());
    let a = cx.new(HttpSession::new);
    let b = cx.new(HttpSession::new);
    for s in [&a, &b] {
        s.update(cx, |s, cx| {
            s.publication_held.store(true, Ordering::Release);
            s.set_draft(draft(&fixture), cx);
            s.send(cx);
        });
    }
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 2);
    a.update(cx, |s, cx| s.retire(cx));
    tick(cx);
    a.update(cx, |s, _| {
        assert!(s.retired);
        assert!(s.draft.url.is_empty());
        assert!(!s.can_copy());
    });
    b.update(cx, |s, _| assert!(!s.retired));
    cx.update(shutdown::request_quit);
    tick(cx);
    for s in [&a, &b] {
        s.update(cx, |s, _| {
            assert!(s.retired);
            assert!(!s.can_copy());
            s.publication_held.store(false, Ordering::Release);
        });
    }
    tick(cx);
    assert_eq!(cx.update(shutdown::registry).active(), 0);
    for s in [&a, &b] {
        s.update(cx, |s, _| {
            assert!(!s.running);
            assert!(s.result.is_none());
        });
    }
}
#[gpui::test]
fn failure_can_retry_and_rejected_draft_cannot_send(cx: &mut TestAppContext) {
    let fixture = server(|stream, index| {
        if index > 0 {
            let _ = stream.write_all(
                b"HTTP/1.1 500 Synthetic\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
            );
        }
    });
    let s = cx.new(HttpSession::new);
    s.update(cx, |s, cx| {
        s.set_draft(draft(&fixture), cx);
        s.send(cx);
    });
    tick(cx);
    s.update(cx, |s, cx| {
        assert!(s.error.is_some());
        assert!(s.can_send());
        s.send(cx);
    });
    tick(cx);
    s.update(cx, |s, cx| {
        assert!(s.can_copy());
        s.reject("Invalid import".into(), cx);
        s.send(cx);
        assert!(!s.can_send());
        assert!(!s.can_copy());
    });
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 2);
}
#[gpui::test]
fn independent_windows_and_fresh_sessions_have_no_request_history(cx: &mut TestAppContext) {
    let fixture = fixed(200, "", b"synthetic result".to_vec());
    let a = cx.new(HttpSession::new);
    let b = cx.new(HttpSession::new);
    a.update(cx, |s, cx| {
        s.set_draft(draft(&fixture), cx);
        s.send(cx);
    });
    tick(cx);
    b.update(cx, |s, _| {
        assert!(s.draft.url.is_empty());
        assert!(s.result.is_none());
        assert!(!s.can_send());
    });
    a.update(cx, |s, cx| s.retire(cx));
    let fresh = cx.new(HttpSession::new);
    fresh.update(cx, |s, _| {
        assert_eq!(s.draft, HttpRequestDraft::default());
        assert!(s.result.is_none());
    });
    assert_eq!(fixture.count.load(Ordering::Acquire), 1);
}
#[gpui::test]
fn revision_exhaustion_retires_without_network(cx: &mut TestAppContext) {
    let fixture = fixed(200, "", Vec::new());
    let s = cx.new(HttpSession::new);
    s.update(cx, |s, cx| {
        s.revision = u64::MAX;
        s.set_draft(draft(&fixture), cx);
        s.send(cx);
        assert!(s.retired);
        assert!(!s.can_send());
    });
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 0);
}
