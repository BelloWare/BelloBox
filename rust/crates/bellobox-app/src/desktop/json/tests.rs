use super::*;
use crate::json_session::Mode;
use gpui::{AppContext, Focusable, TestAppContext};
use std::{sync::atomic::Ordering::SeqCst, time::Duration};
fn tick(cx: &mut TestAppContext, ms: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}
#[gpui::test]
fn failed_and_closed_destination_keep_source_owner_and_draft(cx: &mut TestAppContext) {
    let s = cx.new(|cx| JsonSession::new("[9007199254740993]".into(), true, cx));
    cx.update(|cx| {
        assert!(
            open_with(
                s.clone(),
                cx,
                |_, _| Err("injected creation failure".into())
            )
            .is_err()
        );
        assert!(s.read(cx).can_transfer());
        assert_eq!(s.read(cx).input(), "[9007199254740993]");
        assert!(
            open_with(s.clone(), cx, |s, cx| {
                let h = create_window(s, cx)?;
                h.update(cx, |_, w, _| w.remove_window()).unwrap();
                Ok(h)
            })
            .is_err()
        );
    });
    cx.run_until_parked();
    s.update(cx, |s, _| {
        assert!(s.can_transfer());
        assert!(s.busy);
    });
    tick(cx, 221);
    s.update(cx, |s, _| {
        assert!(s.can_copy());
        assert!(s.output.contains("9007199254740993"));
    });
}
#[gpui::test]
fn rejected_oversized_palette_cannot_open_stale_input(cx: &mut TestAppContext) {
    let s = cx.new(|cx| JsonSession::new("{}".into(), true, cx));
    s.update(cx, |s, cx| {
        s.set_input("x".repeat(bellobox_core::MAX_INPUT_BYTES + 1), cx)
    });
    cx.update(|cx| {
        assert!(!s.read(cx).can_transfer());
        assert!(
            open_with(s.clone(), cx, |_, _| panic!(
                "must refuse before window creation"
            ))
            .is_err()
        );
    });
}
#[gpui::test]
fn actual_full_window_adopts_pending_session_once_and_close_is_independent(
    cx: &mut TestAppContext,
) {
    let s = cx.new(|cx| JsonSession::new("{\"n\":9007199254740993}".into(), true, cx));
    s.update(cx, |s, cx| s.set_mode(Mode::Minify, cx));
    let probe = s.read_with(cx, |s, _| s.worker_probe());
    probe.held.store(true, SeqCst);
    tick(cx, 221);
    assert_eq!(probe.starts.load(SeqCst), 1);
    cx.update(|cx| open(s.clone(), cx).unwrap());
    let first = cx.windows()[0].downcast::<BelloBox>().unwrap();
    first
        .update(cx, |v, _, cx| {
            assert_eq!(v.json.as_ref().unwrap().entity_id(), s.entity_id());
            assert_eq!(v.input.read(cx).text(), "{\"n\":9007199254740993}");
            assert_eq!(v.second.read(cx).text(), "minify");
            assert!(v.busy);
        })
        .unwrap();
    cx.update(|cx| assert!(open(s.clone(), cx).is_err()));
    assert_eq!(cx.windows().len(), 1);
    let second = cx.add_window(|w, cx| BelloBox::new_for("json".into(), "[2]".into(), w, cx));
    tick(cx, 221);
    second
        .update(cx, |v, _, cx| {
            assert_ne!(v.json.as_ref().unwrap().entity_id(), s.entity_id());
            assert_eq!(v.second.read(cx).text(), "pretty");
            assert!(v.output.read(cx).text().contains('2'));
        })
        .unwrap();
    probe.held.store(false, SeqCst);
    tick(cx, 11);
    first
        .update(cx, |v, _, cx| {
            assert_eq!(v.output.read(cx).text(), "{\"n\":9007199254740993}");
            assert!(!v.busy);
        })
        .unwrap();
    assert_eq!(probe.starts.load(SeqCst), 1);
    first.update(cx, |_, w, _| w.remove_window()).unwrap();
    cx.run_until_parked();
    s.update(cx, |s, cx| {
        assert!(!s.can_copy());
        s.set_input("{}".into(), cx);
        assert!(!s.busy);
    });
    second
        .update(cx, |v, _, cx| {
            assert!(v.json.as_ref().unwrap().read(cx).can_copy())
        })
        .unwrap();
    second.update(cx, |_, w, _| w.remove_window()).unwrap();
    let fresh = cx.add_window(|w, cx| BelloBox::new_for("json".into(), String::new(), w, cx));
    fresh
        .update(cx, |v, _, cx| {
            assert!(v.input.read(cx).text().is_empty());
            assert_eq!(v.second.read(cx).text(), "pretty");
        })
        .unwrap();
}
#[gpui::test]
fn full_json_options_invalidate_results_and_validate_cannot_chain(cx: &mut TestAppContext) {
    let full =
        cx.add_window(|w, cx| BelloBox::new_for("json".into(), "{\"b\":2,\"a\":1}".into(), w, cx));
    tick(cx, 221);
    full.update(cx, |v, _, cx| {
        assert!(v.json.as_ref().unwrap().read(cx).can_chain());
        v.choose("validate", cx);
    })
    .unwrap();
    cx.run_until_parked();
    full.update(cx, |v, _, cx| {
        assert!(v.busy);
        assert!(v.output.read(cx).text().is_empty());
    })
    .unwrap();
    tick(cx, 221);
    full.update(cx, |v, _, cx| {
        assert!(!v.busy);
        assert!(v.json.as_ref().unwrap().read(cx).can_copy());
        assert!(!v.json.as_ref().unwrap().read(cx).can_chain());
    })
    .unwrap();
}

#[gpui::test]
fn quit_during_destination_creation_refuses_transfer(cx: &mut TestAppContext) {
    let s = cx.new(|cx| JsonSession::new("[1]".into(), true, cx));
    cx.update(|cx| {
        let result = open_with(s.clone(), cx, |s, cx| {
            let h = create_window(s, cx)?;
            crate::shutdown::request_quit(cx);
            Ok(h)
        });
        assert!(result.is_err());
        assert!(s.read(cx).can_transfer());
        assert_eq!(s.read(cx).input(), "[1]");
    });
}

#[gpui::test]
fn full_oversized_editor_draft_is_retained_but_never_copied_into_the_session(
    cx: &mut TestAppContext,
) {
    let full = cx.add_window(|w, cx| BelloBox::new_for("json".into(), "[1]".into(), w, cx));
    tick(cx, 221);
    full.update(cx, |v, w, cx| {
        assert!(v.input.read(cx).focus_handle(cx).is_focused(w));
        v.input.update(cx, |e, cx| {
            e.set_text("x".repeat(bellobox_core::MAX_INPUT_BYTES + 1), cx)
        });
        v.run_tool(cx);
        let s = v.json.as_ref().unwrap().read(cx);
        assert_eq!(
            v.input.read(cx).text().len(),
            bellobox_core::MAX_INPUT_BYTES + 1
        );
        assert_eq!(s.input(), "[1]");
        assert!(!s.can_copy());
        assert!(!s.can_transfer());
        assert!(s.error.is_some());
        v.choose("validate", cx);
    })
    .unwrap();
    tick(cx, 221);
    full.update(cx, |v, _, cx| {
        assert!(v.error.is_some());
        assert!(!v.busy);
        assert!(v.output.read(cx).text().is_empty());
    })
    .unwrap();
}
