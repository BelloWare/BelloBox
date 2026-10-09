use super::*;
use crate::text_tool_state::{Choices, TextSession};
use gpui::{AppContext, Focusable, TestAppContext};
use std::time::Duration;
fn tick(cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(301));
    cx.run_until_parked();
}
fn handoff() -> Handoff {
    let mut choices = Choices::default();
    choices.select_option("snake");
    Handoff {
        input: "hello World".into(),
        choices,
    }
}
#[gpui::test]
fn text_open_failures_preserve_independent_palette_snapshot(cx: &mut TestAppContext) {
    let source = cx.new(|cx| TextSession::new("hello World".into(), handoff().choices, true, cx));
    cx.update(|cx| {
        assert!(
            open_with(source.read(cx).snapshot().unwrap(), cx, |_, _| Err(
                "injected".into()
            ))
            .is_err()
        );
        assert!(source.read(cx).can_snapshot());
        assert!(
            open_with(source.read(cx).snapshot().unwrap(), cx, |h, cx| {
                let w = create_window(h, cx)?;
                w.update(cx, |_, w, _| w.remove_window()).unwrap();
                Ok(w)
            })
            .is_err()
        );
    });
    tick(cx);
    source.update(cx, |s, _| {
        assert!(s.can_snapshot());
        assert_eq!(s.output, "hello_world");
    });
}
#[gpui::test]
fn text_quit_during_admission_and_oversize_are_refused(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert!(
            open_with(
                Handoff {
                    input: "x".repeat(bellobox_core::MAX_INPUT_BYTES + 1),
                    choices: Choices::default()
                },
                cx,
                |_, _| panic!("oversize admitted")
            )
            .is_err()
        );
        assert!(
            open_with(handoff(), cx, |h, cx| {
                let w = create_window(h, cx)?;
                crate::shutdown::request_quit(cx);
                Ok(w)
            })
            .is_err()
        );
    });
}
#[gpui::test]
fn text_popup_snapshot_focus_options_chain_reset_and_independence(cx: &mut TestAppContext) {
    let source = cx.new(|cx| TextSession::new("hello World".into(), handoff().choices, true, cx));
    cx.update(|cx| open(source.read(cx).snapshot().unwrap(), cx).unwrap());
    let first = cx.windows()[0].downcast::<BelloBox>().unwrap();
    let second =
        cx.add_window(|w, cx| BelloBox::new_for("textTools".into(), "keep me".into(), w, cx));
    tick(cx);
    first
        .update(cx, |v, w, cx| {
            assert!(v.input.read(cx).focus_handle(cx).is_focused(w));
            assert_ne!(
                v.text_session.as_ref().unwrap().entity_id(),
                source.entity_id()
            );
            assert_eq!(v.output.read(cx).text(), "hello_world");
            v.text_category(Category::Encode, cx);
            v.text_option("url", cx);
            v.text_category(Category::Case, cx);
            assert_eq!(
                v.text_session.as_ref().unwrap().read(cx).choices.argument(),
                "snake"
            );
        })
        .unwrap();
    tick(cx);
    source.update(cx, |s, cx| s.retire(cx));
    first
        .update(cx, |v, _, cx| {
            v.text_chain(cx);
        })
        .unwrap();
    tick(cx);
    first
        .update(cx, |v, _, cx| {
            assert_eq!(v.input.read(cx).text(), "hello_world");
            v.input
                .update(cx, |e, cx| e.set_text(v.initial_text.clone(), cx));
        })
        .unwrap();
    tick(cx);
    first
        .update(cx, |v, w, cx| {
            assert_eq!(v.input.read(cx).text(), "hello World");
            assert_eq!(v.output.read(cx).text(), "hello_world");
            w.remove_window();
        })
        .unwrap();
    cx.run_until_parked();
    second
        .update(cx, |v, _, cx| {
            assert_eq!(v.input.read(cx).text(), "keep me");
            assert_eq!(v.output.read(cx).text(), "KEEP ME");
        })
        .unwrap();
}
#[gpui::test]
fn text_busy_error_and_oversized_input_never_copy_previous_result(cx: &mut TestAppContext) {
    let full = cx.add_window(|w, cx| BelloBox::new_for("textTools".into(), "hello".into(), w, cx));
    tick(cx);
    full.update(cx, |v, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
        v.text_category(Category::Decode, cx);
        v.text_option("decode-hex", cx);
        v.text_copy(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("sentinel")
        );
        assert!(v.output.read(cx).text().is_empty());
    })
    .unwrap();
    tick(cx);
    full.update(cx, |v, _, cx| {
        assert!(v.error.is_some());
        v.text_copy(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("sentinel")
        );
        v.input.update(cx, |e, cx| {
            e.set_text("x".repeat(bellobox_core::MAX_INPUT_BYTES + 1), cx)
        });
        v.run_tool(cx);
        assert_eq!(
            v.input.read(cx).text().len(),
            bellobox_core::MAX_INPUT_BYTES + 1
        );
        assert_eq!(v.text_session.as_ref().unwrap().read(cx).input(), "hello");
        assert!(v.error.is_some());
        v.text_category(Category::Case, cx);
        v.text_copy(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("sentinel")
        );
    })
    .unwrap();
}

#[gpui::test]
fn text_closed_destination_retires_even_when_entity_is_retained(cx: &mut TestAppContext) {
    let mut retained = None;
    cx.update(|cx| {
        assert!(
            open_with(handoff(), cx, |h, cx| {
                let w = create_window(h, cx)?;
                retained = Some(
                    w.update(cx, |v, w, _| {
                        let s = v.text_session.clone().unwrap();
                        w.remove_window();
                        s
                    })
                    .unwrap(),
                );
                Ok(w)
            })
            .is_err()
        );
    });
    tick(cx);
    retained.unwrap().update(cx, |s, cx| {
        assert!(!s.can_copy());
        assert!(!s.busy);
        s.set_input("must stay retired".into(), cx);
        assert!(!s.busy);
        assert_ne!(s.input(), "must stay retired");
    });
}
#[gpui::test]
fn text_popup_tab_and_held_activation_do_not_type_or_double_chain(cx: &mut TestAppContext) {
    let full = cx.add_window(|w, cx| BelloBox::new_for("textTools".into(), "hello".into(), w, cx));
    tick(cx);
    full.update(cx, |v, w, cx| {
        let event = |k: &str, held| gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse(k).unwrap(),
            is_held: held,
        };
        assert!(v.text_key(&event("tab", false), w, cx));
        assert!(v.text_focus[0].is_focused(w));
        assert!(v.text_key(&event("space", false), w, cx));
        assert!(v.text_key(&event("space", true), w, cx));
        assert_eq!(v.input.read(cx).text(), "hello");
        assert_eq!(
            v.text_session.as_ref().unwrap().read(cx).choices.category,
            Category::Case
        );
        assert!(v.text_key_up(
            &gpui::KeyUpEvent {
                keystroke: gpui::Keystroke::parse("space").unwrap()
            },
            w,
            cx
        ));
        v.text_focus[17].focus(w);
        assert!(v.text_key(&event("enter", false), w, cx));
        assert!(v.text_key(&event("enter", true), w, cx));
    })
    .unwrap();
    tick(cx);
    full.update(cx, |v, _, cx| assert_eq!(v.input.read(cx).text(), "HELLO"))
        .unwrap();
}

#[gpui::test]
fn text_explicit_paste_keeps_rejected_editor_draft_and_fences_previous_copy(
    cx: &mut TestAppContext,
) {
    let full =
        cx.add_window(|w, cx| BelloBox::new_for("textTools".into(), "original".into(), w, cx));
    tick(cx);
    full.update(cx, |v, _, cx| {
        let rejected = "x".repeat(bellobox_core::MAX_INPUT_BYTES + 1);
        cx.write_to_clipboard(ClipboardItem::new_string(rejected.clone()));
        v.text_paste(cx);
        assert_eq!(v.input.read(cx).text(), rejected);
        assert!(v.error.is_some());
        assert!(v.output.read(cx).text().is_empty());
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
        v.text_copy(cx);
        v.text_chain(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("sentinel")
        );
        assert_eq!(v.input.read(cx).text(), rejected);
        v.text_category(Category::Count, cx);
        assert!(v.error.is_some());
        assert!(!v.text_session.as_ref().unwrap().read(cx).can_copy());
        v.text_act(16, cx);
    })
    .unwrap();
    tick(cx);
    full.update(cx, |v, _, cx| {
        assert_eq!(v.input.read(cx).text(), "original");
        assert!(v.error.is_none());
        assert!(v.text_session.as_ref().unwrap().read(cx).can_copy());
    })
    .unwrap();
}
