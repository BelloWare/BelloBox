use super::*;
use gpui::{AppContext, TestAppContext};
fn tick(cx: &mut TestAppContext, ms: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}
#[gpui::test]
fn source_defaults_idle_pattern_notice_and_modes_reuse_one_result(cx: &mut TestAppContext) {
    let session = cx.new(RegexSession::new);
    session.update(cx, |s, cx| {
        assert_eq!(
            s.options,
            Options {
                ignore_case: false,
                multiline: true
            }
        );
        s.set_drafts(" \n".into(), "a".into(), "!".into(), cx);
        assert!(!s.busy);
        s.set_drafts("a\nb".into(), String::new(), "!".into(), cx);
        assert!(!s.busy);
        assert!(s.status.contains("Enter a regular expression"));
        s.set_drafts("a\nb".into(), "(a)".into(), "[$1]".into(), cx);
    });
    tick(cx, 221);
    session.update(cx, |s, cx| {
        assert!(s.can_copy());
        assert!(!s.can_chain());
        let token = s.ready;
        assert_eq!(s.result.as_ref().unwrap().details, "1. [0..<1] a\n   $1: a");
        s.set_mode(Mode::Extract, cx);
        assert_eq!(s.ready, token);
        assert!(s.can_chain());
        assert!(!s.busy);
        assert_eq!(s.result.as_ref().unwrap().output(s.mode), "a");
        s.set_mode(Mode::Replace, cx);
        assert_eq!(s.ready, token);
        assert!(s.can_chain());
        assert!(!s.busy);
        assert_eq!(s.result.as_ref().unwrap().output(s.mode), "[a]\nb");
    });
}
#[gpui::test]
fn one_physical_worker_coalesces_all_drafts_and_options(cx: &mut TestAppContext) {
    use std::sync::atomic::Ordering::SeqCst;
    let session = cx.new(RegexSession::new);
    let probe = session.read_with(cx, |s, _| s.worker_probe());
    probe.held.store(true, SeqCst);
    session.update(cx, |s, cx| {
        s.set_drafts("a".into(), "a".into(), "old".into(), cx)
    });
    tick(cx, 221);
    assert_eq!(probe.starts.load(SeqCst), 1);
    session.update(cx, |s, cx| {
        for i in 0..100 {
            s.set_drafts("A".into(), "(a)".into(), i.to_string(), cx);
        }
        s.set_options(
            Options {
                ignore_case: true,
                multiline: false,
            },
            cx,
        );
        s.set_mode(Mode::Replace, cx);
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
        assert_eq!(s.result.as_ref().unwrap().replaced, "99");
    });
}
#[gpui::test]
fn edit_reject_cancel_close_and_new_window_fence_late_results(cx: &mut TestAppContext) {
    let session = cx.new(RegexSession::new);
    let other = cx.new(RegexSession::new);
    let result = || {
        inspect(
            "a",
            "a",
            "b",
            Options::default(),
            &std::sync::atomic::AtomicBool::new(false),
        )
    };
    let foreign = other.update(cx, |s, cx| {
        s.set_drafts("a".into(), "a".into(), "b".into(), cx);
        s.desired.unwrap()
    });
    session.update(cx, |s, cx| {
        s.set_drafts("a".into(), "a".into(), "b".into(), cx);
        let old = s.desired.unwrap();
        s.set_drafts("new".into(), "n".into(), "z".into(), cx);
        s.publish(old, result(), cx);
        s.publish(foreign, result(), cx);
        assert!(s.result.is_none());
        let old = s.desired.unwrap();
        s.reject("Rejected oversized paste".into(), cx);
        s.publish(old, result(), cx);
        s.refresh(cx);
        s.set_options(
            Options {
                ignore_case: true,
                multiline: true,
            },
            cx,
        );
        assert!(s.result.is_none());
        assert!(s.desired.is_none());
        s.set_drafts("a".into(), "a".into(), "b".into(), cx);
        let old = s.desired.unwrap();
        s.cancel(cx);
        s.publish(old, result(), cx);
        assert!(!s.can_copy());
        assert!(s.result.is_none());
        s.set_drafts("a".into(), "a".into(), "b".into(), cx);
        let old = s.desired.unwrap();
        s.retire(cx);
        s.publish(old, result(), cx);
        s.set_drafts("new".into(), "n".into(), "".into(), cx);
        assert!(s.desired.is_none());
        assert!(!s.can_copy());
    });
    tick(cx, 221);
    other.update(cx, |s, _| assert!(s.can_copy()));
}
#[gpui::test]
fn closing_held_worker_drains_without_publishing(cx: &mut TestAppContext) {
    use std::sync::atomic::Ordering::SeqCst;
    let s = cx.new(RegexSession::new);
    let probe = s.read_with(cx, |s, _| s.worker_probe());
    probe.held.store(true, SeqCst);
    s.update(cx, |s, cx| {
        s.set_drafts("a".into(), "a".into(), "b".into(), cx)
    });
    tick(cx, 221);
    s.update(cx, |s, cx| s.retire(cx));
    probe.held.store(false, SeqCst);
    tick(cx, 11);
    s.update(cx, |s, _| {
        assert!(!s.running);
        assert!(s.result.is_none());
        assert!(!s.can_copy());
    });
}
#[gpui::test]
fn empty_unchanged_and_oversized_outputs_cannot_chain(cx: &mut TestAppContext) {
    let s = cx.new(RegexSession::new);
    s.update(cx, |s, cx| {
        s.set_drafts("a".into(), "z".into(), "".into(), cx);
        s.set_mode(Mode::Extract, cx);
    });
    tick(cx, 221);
    s.update(cx, |s, cx| {
        assert!(!s.can_copy());
        assert!(!s.can_chain());
        s.set_mode(Mode::Replace, cx);
        assert!(s.can_copy());
        assert!(!s.can_chain());
        s.set_drafts("aa".into(), "a".into(), "x".repeat(60000), cx);
    });
    tick(cx, 221);
    s.update(cx, |s, _| {
        assert!(s.can_copy());
        assert!(!s.can_chain());
        assert_eq!(s.result.as_ref().unwrap().replaced.len(), 120000);
    });
}
#[gpui::test]
fn revision_exhaustion_is_sticky(cx: &mut TestAppContext) {
    let s = cx.new(RegexSession::new);
    s.update(cx, |s, cx| {
        s.revision = u64::MAX - 1;
        s.set_drafts("a".into(), "a".into(), "".into(), cx);
        s.set_mode(Mode::Replace, cx);
        assert!(s.retired);
        assert!(s.result.is_none());
        assert!(!s.busy);
        s.refresh(cx);
        assert!(s.error.as_ref().unwrap().contains("expired"));
    });
}
