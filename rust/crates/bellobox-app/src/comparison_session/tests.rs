use super::*;
use bellobox_core::developer::comparison::Mode;
use gpui::{AppContext, TestAppContext};
fn tick(cx: &mut TestAppContext, ms: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}
#[gpui::test]
fn either_side_admission_and_source_blank_idle_gate(cx: &mut TestAppContext) {
    let session = cx.new(ComparisonSession::new);
    session.update(cx, |s, cx| s.set_drafts(String::new(), "b".into(), cx));
    tick(cx, 221);
    session.update(cx, |s, cx| {
        assert!(s.can_copy());
        assert_eq!(s.result.as_ref().unwrap().copy_text, "+ b");
        s.set_drafts("a".into(), String::new(), cx);
        assert!(!s.can_copy());
        assert!(s.result.is_none());
    });
    tick(cx, 221);
    session.update(cx, |s, cx| {
        assert_eq!(s.result.as_ref().unwrap().copy_text, "− a");
        s.set_drafts(" \n".into(), String::new(), cx);
        assert!(!s.busy);
        assert!(s.result.is_none());
        s.set_drafts(String::new(), " \n".into(), cx);
        assert!(s.busy);
    });
    tick(cx, 221);
    session.update(cx, |s, cx| {
        assert_eq!(s.result.as_ref().unwrap().added, 2);
        s.set_drafts(String::new(), String::new(), cx);
        assert!(!s.can_copy());
        assert!(!s.busy);
    });
}
#[gpui::test]
fn one_physical_worker_coalesces_and_publishes_only_latest(cx: &mut TestAppContext) {
    use std::sync::atomic::Ordering::SeqCst;
    let session = cx.new(ComparisonSession::new);
    let probe = session.read_with(cx, |s, _| s.worker_probe());
    probe.held.store(true, SeqCst);
    session.update(cx, |s, cx| s.set_drafts("a".into(), "b".into(), cx));
    tick(cx, 221);
    assert_eq!(probe.starts.load(SeqCst), 1);
    session.update(cx, |s, cx| {
        for i in 0..100 {
            s.set_drafts("a".into(), i.to_string(), cx);
        }
        s.set_options(
            Options {
                mode: Mode::Words,
                ignore_whitespace: true,
            },
            cx,
        );
        assert!(s.running);
        assert!(s.busy);
        assert!(!s.can_copy());
        assert!(s.result.is_none());
    });
    tick(cx, 1000);
    assert_eq!(probe.starts.load(SeqCst), 1);
    probe.held.store(false, SeqCst);
    tick(cx, 11);
    tick(cx, 221);
    assert_eq!(probe.starts.load(SeqCst), 2);
    session.update(cx, |s, _| {
        assert!(s.can_copy());
        assert!(!s.running);
        assert!(!s.busy);
        assert_eq!(s.result.as_ref().unwrap().copy_text, "− a\n+ 99");
    });
}
#[gpui::test]
fn options_rejection_cancel_and_close_fence_old_completion(cx: &mut TestAppContext) {
    let session = cx.new(ComparisonSession::new);
    session.update(cx, |s, cx| {
        s.set_drafts("a".into(), "b".into(), cx);
        let old = s.desired.unwrap();
        let result = || {
            compare(
                "a",
                "b",
                Options::default(),
                &std::sync::atomic::AtomicBool::new(false),
            )
        };
        s.set_options(
            Options {
                mode: Mode::Words,
                ignore_whitespace: false,
            },
            cx,
        );
        s.publish(old, result(), cx);
        assert!(s.result.is_none());
        let next = s.desired.unwrap();
        s.reject("Rejected oversized paste".into(), cx);
        s.publish(next, result(), cx);
        assert!(s.result.is_none());
        assert!(s.error.is_some());
        s.set_options(Options::default(), cx);
        s.refresh(cx);
        assert!(s.desired.is_none());
        s.set_drafts("a".into(), "b".into(), cx);
        let cancelled = s.desired.unwrap();
        s.cancel(cx);
        s.publish(cancelled, result(), cx);
        assert!(!s.can_copy());
        assert!(s.result.is_none());
        s.set_drafts("a".into(), "c".into(), cx);
        let closed = s.desired.unwrap();
        s.retire(cx);
        s.publish(closed, result(), cx);
        assert!(!s.can_copy());
        s.set_drafts("new".into(), "draft".into(), cx);
        assert!(s.desired.is_none());
    });
}
#[gpui::test]
fn held_worker_is_cancelled_on_close_and_fresh_session_is_independent(cx: &mut TestAppContext) {
    use std::sync::atomic::Ordering::SeqCst;
    let a = cx.new(ComparisonSession::new);
    let b = cx.new(ComparisonSession::new);
    let probe = a.read_with(cx, |s, _| s.worker_probe());
    probe.held.store(true, SeqCst);
    a.update(cx, |s, cx| s.set_drafts("a".into(), "b".into(), cx));
    b.update(cx, |s, cx| s.set_drafts("b".into(), "c".into(), cx));
    tick(cx, 221);
    a.update(cx, |s, cx| s.retire(cx));
    b.update(cx, |s, _| assert!(s.can_copy()));
    probe.held.store(false, SeqCst);
    tick(cx, 11);
    a.update(cx, |s, _| {
        assert!(!s.running);
        assert!(s.result.is_none());
    });
    assert_eq!(probe.starts.load(SeqCst), 1);
}
#[gpui::test]
fn combined_limit_never_calculates_retained_old_drafts(cx: &mut TestAppContext) {
    let s = cx.new(ComparisonSession::new);
    s.update(cx, |s, cx| s.set_drafts("a".into(), "b".into(), cx));
    tick(cx, 221);
    s.update(cx, |s, cx| {
        assert!(s.can_copy());
        s.set_drafts("a".repeat(250001), "b".repeat(250000), cx);
        assert!(s.error.is_some());
        assert!(!s.can_copy());
        assert!(s.result.is_none());
        s.refresh(cx);
        assert!(s.desired.is_none());
        s.set_drafts("a".into(), "a".into(), cx);
    });
    tick(cx, 221);
    s.update(cx, |s, _| {
        assert!(s.can_copy());
        assert_eq!(s.result.as_ref().unwrap().copy_text, "  a");
    });
}
#[gpui::test]
fn presentation_revision_exhaustion_is_sticky_and_cannot_publish(cx: &mut TestAppContext) {
    let s = cx.new(ComparisonSession::new);
    s.update(cx, |s, cx| {
        s.revision = u64::MAX - 1;
        s.set_drafts("a".into(), "b".into(), cx);
        assert_eq!(s.revision, u64::MAX);
        let old = s.desired.unwrap();
        s.set_drafts("b".into(), "c".into(), cx);
        assert!(s.retired);
        assert!(s.desired.is_none());
        assert!(s.error.as_ref().unwrap().contains("expired"));
        s.publish(
            old,
            compare(
                "a",
                "b",
                Options::default(),
                &std::sync::atomic::AtomicBool::new(false),
            ),
            cx,
        );
        assert!(!s.can_copy());
        assert!(s.result.is_none());
        s.refresh(cx);
        s.set_drafts("new".into(), "draft".into(), cx);
        assert!(s.desired.is_none());
        assert_eq!(s.revision, u64::MAX);
    });
}

#[gpui::test]
fn accepted_error_retires_working_status_and_recovers(cx: &mut TestAppContext) {
    let s = cx.new(ComparisonSession::new);
    s.update(cx, |s, cx| {
        s.set_options(
            Options {
                mode: Mode::Json,
                ignore_whitespace: false,
            },
            cx,
        );
        s.set_drafts("{\"a\":}".into(), "{}".into(), cx);
        assert_eq!(s.status, "Working locally…");
    });
    tick(cx, 221);
    s.update(cx, |s, cx| {
        assert!(s.error.is_some());
        assert!(s.status.is_empty());
        assert!(!s.busy);
        assert!(!s.can_copy());
        assert!(s.result.is_none());
        s.set_drafts("{}".into(), "{}".into(), cx);
    });
    tick(cx, 221);
    s.update(cx, |s, _| {
        assert!(s.error.is_none());
        assert_eq!(s.status, "0 added · 0 removed");
        assert!(s.can_copy());
    });
}
