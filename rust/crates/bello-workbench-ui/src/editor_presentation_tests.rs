use super::{Editor, EditorEvent, EditorView, PresentationError, TextPresentation};
use crate::TextDecoration;
use gpui::{IntoElement, TestAppContext, point, px, rgb, size};
use std::ops::Range;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

fn presentation(text: &str, token: u64, range: Range<usize>) -> TextPresentation {
    TextPresentation {
        token,
        text: Arc::from(text),
        decorations: vec![TextDecoration {
            range: range.clone(),
            color: rgb(0xffff00).into(),
        }]
        .into(),
        emphasized: Some(TextDecoration {
            range,
            color: rgb(0xff9900).into(),
        }),
    }
}

#[gpui::test]
fn decorations_preserve_focus_selection_vim_search_and_open_undo_group(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| EditorView::new("😀\r\nstart".into(), w, cx));
    let changed = Rc::new(Cell::new(0));
    view.update(cx, |ed, w, cx| {
        let events = changed.clone();
        cx.subscribe(&cx.entity(), move |_, _, event, _| {
            if matches!(event, EditorEvent::Changed) {
                events.set(events.get() + 1);
            }
        })
        .detach();
        let other_focus = cx.focus_handle();
        other_focus.focus(w);
        ed.engine.set_cursor(ed.text().len());
        ed.engine.insert_text("甲");
        ed.selection = Some(0..4);
        ed.engine.search = "retained Vim search".into();
        let revision = ed.engine.revision();
        let cursor = ed.engine.cursor;
        let text = ed.text().to_owned();
        ed.set_text_presentation(Some(presentation(&text, 7, 6..11)), cx)
            .unwrap();
        ed.reveal_presented_range(7, 6..11, cx).unwrap();
        ed.cancel_presentation_reveal(6, cx); // wrong token cannot cancel new work
        assert!(ed.presentation_reveal.is_some());
        ed.set_text_presentation(None, cx).unwrap();
        assert_eq!(ed.engine.revision(), revision);
        assert_eq!(ed.engine.cursor, cursor);
        assert_eq!(ed.selection, Some(0..4));
        assert_eq!(ed.engine.search, "retained Vim search");
        assert!(other_focus.is_focused(w));
        assert!(!ed.focus.is_focused(w));
        ed.engine.insert_text("乙");
        ed.engine.undo();
        assert_eq!(ed.text(), "😀\r\nstart");
        ed.engine.redo();
        assert_eq!(ed.text(), "😀\r\nstart甲乙");
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(changed.get(), 0);
}

#[gpui::test]
fn composition_drag_rejection_and_atomic_validation_keep_input(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| EditorView::new("😀に尾".into(), w, cx));
    view.update(cx, |ed, _, cx| {
        ed.marked = Some(4..7);
        ed.selection = Some(7..7);
        ed.engine.cursor = 7;
        ed.set_text_presentation(Some(presentation(ed.text(), 1, 4..7)), cx)
            .unwrap();
        assert_eq!(
            ed.reveal_presented_range(1, 4..7, cx),
            Err(PresentationError::Busy)
        );
        assert_eq!(
            ed.measure_presented_range(1, 4..7, 99, |_, _, _| panic!("busy callback"), cx),
            Err(PresentationError::Busy)
        );
        assert_eq!(ed.marked, Some(4..7));
        assert_eq!(ed.selection, Some(7..7));
        assert_eq!(ed.engine.cursor, 7);
        assert_eq!(
            ed.set_text_presentation(Some(presentation("different", 2, 0..1)), cx),
            Err(PresentationError::TextChanged)
        );
        assert_eq!(ed.presentation.as_ref().unwrap().token, 1);
        let mut invalid = presentation(ed.text(), 2, 2..4);
        assert_eq!(
            ed.set_text_presentation(Some(invalid.clone()), cx),
            Err(PresentationError::InvalidRange)
        );
        invalid.decorations = Arc::from([]);
        invalid.emphasized = None;
        ed.marked = None;
        ed.dragging = Some(0);
        assert_eq!(
            ed.reveal_presented_range(1, 4..7, cx),
            Err(PresentationError::Busy)
        );
        ed.set_text_presentation(None, cx).unwrap();
        assert_eq!(ed.dragging, Some(0));
        assert_eq!(ed.text(), "😀に尾");
    })
    .unwrap();
}

#[gpui::test]
fn replacement_aba_and_newest_request_cancel_transient_state(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| EditorView::new("original".into(), w, cx));
    view.update(cx, |ed, _, cx| {
        ed.set_text_presentation(Some(presentation(ed.text(), 1, 0..3)), cx)
            .unwrap();
        ed.measure_presented_range(1, 0..3, 1, |_, _, _| panic!("old callback"), cx)
            .unwrap();
        ed.set_text("original".into(), cx);
        assert!(ed.presentation.is_none());
        assert!(ed.presentation_measurement.is_none());
        ed.set_text_presentation(Some(presentation(ed.text(), 2, 0..3)), cx)
            .unwrap();
        ed.reveal_presented_range(2, 0..3, cx).unwrap();
        ed.measure_presented_range(2, 0..3, 2, |_, _, _| {}, cx)
            .unwrap();
        let serial = ed.presentation_measurement.as_ref().unwrap().serial;
        ed.measure_presented_range(2, 4..8, 3, |_, _, _| {}, cx)
            .unwrap();
        assert_ne!(ed.presentation_measurement.as_ref().unwrap().serial, serial);
        ed.cancel_presentation_reveal(1, cx);
        assert!(ed.presentation_measurement.is_some());
        ed.cancel_presentation_reveal(2, cx);
        assert!(ed.presentation_measurement.is_none());
        ed.engine = Editor::new("replaced".into()); // same length, revision reset
        assert_eq!(
            ed.reveal_presented_range(2, 0..3, cx),
            Err(PresentationError::TextChanged)
        );
    })
    .unwrap();
}

#[gpui::test]
fn actual_paint_measures_unicode_without_edit_or_focus(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|w, cx| EditorView::new("before 😀é after".into(), w, cx));
    let result = Rc::new(RefCell::new(None));
    view.update(cx, |ed, cx| {
        let sink = result.clone();
        ed.set_text_presentation(Some(presentation(ed.text(), 8, 7..14)), cx)
            .unwrap();
        ed.measure_presented_range(
            8,
            7..14,
            77,
            move |g, _, _| *sink.borrow_mut() = Some(g),
            cx,
        )
        .unwrap();
    });
    cx.draw(point(px(20.), px(30.)), size(px(350.), px(120.)), |_, _| {
        view.clone().into_any_element()
    });
    cx.run_until_parked();
    let g = result.borrow().clone().expect("fresh paint receipt");
    assert_eq!(g.token, 8);
    assert_eq!(g.host_generation, 77);
    assert!(g.first_visible_fragment.size.width > px(0.));
    assert!(g.fully_visible);
    view.read_with(cx, |ed, _| {
        assert_eq!(ed.engine.cursor, 0);
        assert_eq!(ed.engine.revision(), 0);
        assert!(ed.presentation_measurement.is_none());
    });
}

#[gpui::test]
fn cancelled_or_offscreen_measurement_never_fabricates_geometry(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|w, cx| EditorView::new("visible\n".repeat(300), w, cx));
    let called = Rc::new(Cell::new(false));
    view.update(cx, |ed, cx| {
        let sink = called.clone();
        let end = ed.text().len() - 1;
        ed.set_text_presentation(Some(presentation(ed.text(), 1, end - 3..end)), cx)
            .unwrap();
        ed.measure_presented_range(1, end - 3..end, 1, move |_, _, _| sink.set(true), cx)
            .unwrap();
    });
    cx.draw(point(px(0.), px(0.)), size(px(300.), px(100.)), |_, _| {
        view.clone().into_any_element()
    });
    cx.run_until_parked();
    assert!(!called.get());
    view.update(cx, |ed, cx| {
        ed.cancel_presentation_reveal(1, cx);
        ed.reveal_presented_range(1, 2390..2395, cx).unwrap();
        ed.invalidate_presentation_geometry(cx);
        assert!(ed.presentation_measurement.is_none());
    });
    cx.draw(point(px(0.), px(0.)), size(px(300.), px(100.)), |_, _| {
        view.clone().into_any_element()
    });
    cx.run_until_parked();
    assert!(!called.get());
}

#[gpui::test]
fn prepaint_alone_cannot_deliver_a_measurement(cx: &mut TestAppContext) {
    use gpui::{Element, Styled};
    let (view, cx) = cx.add_window_view(|w, cx| EditorView::new("paint only".into(), w, cx));
    let called = Rc::new(Cell::new(false));
    // Arm and directly draw inside one effect cycle, so the ordinary root does
    // not perform an intervening legitimate paint on our behalf.
    cx.update(|_, cx| {
        view.update(cx, |ed, cx| {
            let sink = called.clone();
            ed.set_text_presentation(Some(presentation(ed.text(), 1, 0..5)), cx)
                .unwrap();
            ed.measure_presented_range(1, 0..5, 3, move |_, _, _| sink.set(true), cx)
                .unwrap();
            assert!(!called.get(), "arming cannot return cached geometry");
            ed.cancel_presentation_reveal(1, cx);
        })
    });
    let before = view.clone();
    let after = view.clone();
    let observed = called.clone();
    cx.draw(point(px(0.), px(0.)), size(px(300.), px(100.)), |_, _| {
        gpui::canvas(
            move |bounds, window, cx| {
                before.update(cx, |ed, cx| {
                    let sink = observed.clone();
                    ed.measure_presented_range(1, 0..5, 4, move |_, _, _| sink.set(true), cx)
                        .unwrap();
                });
                let mut line = super::LineElement {
                    editor: before.clone(),
                    row: 0,
                };
                line.prepaint(None, None, bounds, &mut (), window, cx)
            },
            move |_, prepared, _, cx| {
                assert_eq!(prepared.decorations.len(), 2);
                assert!(prepared.measured_fragment.is_some());
                after.update(cx, |ed, cx| {
                    assert!(ed.painted_fragments.is_empty());
                    assert!(ed.measurement_frame_scheduled.is_none());
                    ed.cancel_presentation_reveal(1, cx);
                });
            },
        )
        .size_full()
    });
    cx.run_until_parked();
    assert!(
        !called.get(),
        "prepaint-only canvas must not deliver a receipt"
    );
}

#[gpui::test]
fn reveal_keeps_inner_landing_across_redraw_then_input_cancels(cx: &mut TestAppContext) {
    let text = (0..300)
        .map(|i| format!("row{i} {} TARGET\n", "x".repeat(200)))
        .collect::<String>();
    let target = text.rfind("TARGET").unwrap();
    let (view, cx) = cx.add_window_view(|w, cx| EditorView::new(text, w, cx));
    view.update(cx, |ed, cx| {
        ed.set_text_presentation(Some(presentation(ed.text(), 1, target..target + 6)), cx)
            .unwrap();
        ed.reveal_presented_range(1, target..target + 6, cx)
            .unwrap();
    });
    cx.draw(point(px(0.), px(0.)), size(px(300.), px(100.)), |_, _| {
        view.clone().into_any_element()
    });
    let column = view.read_with(cx, |ed, _| {
        assert_eq!(ed.engine.cursor, 0);
        assert!(ed.horizontal_column > 100);
        assert!(
            ed.scroll.0.borrow().base_handle.offset().y < px(-1000.),
            "offset {:?}",
            ed.scroll.0.borrow().base_handle.offset()
        );
        ed.horizontal_column
    });
    view.update(cx, |_, cx| cx.notify());
    cx.draw(point(px(0.), px(0.)), size(px(300.), px(100.)), |_, _| {
        view.clone().into_any_element()
    });
    view.read_with(cx, |ed, _| assert_eq!(ed.horizontal_column, column));
    cx.update(|window, cx| {
        view.update(cx, |ed, cx| {
            ed.measure_presented_range(1, target..target + 6, 1, |_, _, _| panic!("cancelled"), cx)
                .unwrap();
            ed.key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("right").unwrap(),
                    is_held: false,
                },
                window,
                cx,
            );
            assert!(ed.presentation_measurement.is_none());
            assert!(ed.presentation_reveal.is_none());
            assert!(!ed.presentation_holds_scroll);
        })
    });
}

#[gpui::test]
fn explicit_select_all_is_separate_from_decorations_and_refuses_ime(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| EditorView::new("query 😀".into(), w, cx));
    view.update(cx, |ed, w, cx| {
        let other = cx.focus_handle();
        other.focus(w);
        ed.marked = Some(0..5);
        assert_eq!(ed.select_all(cx), Err(PresentationError::Busy));
        assert_eq!(ed.selection, None);
        ed.marked = None;
        ed.dragging = Some(1);
        assert_eq!(ed.select_all(cx), Err(PresentationError::Busy));
        ed.dragging = None;
        ed.select_all(cx).unwrap();
        assert_eq!(ed.selection, Some(0..10));
        assert_eq!(ed.engine.cursor, 0);
        assert_eq!(ed.engine.revision(), 0);
        assert!(other.is_focused(w));
        ed.set_text_presentation(Some(presentation(ed.text(), 1, 0..5)), cx)
            .unwrap();
        ed.set_text_presentation(None, cx).unwrap();
        assert_eq!(ed.selection, Some(0..10));
    })
    .unwrap();
}

#[gpui::test]
fn clipped_paint_and_public_engine_replacement_cannot_claim_full_visibility(
    cx: &mut TestAppContext,
) {
    use gpui::{Bounds, ContentMask, Element, Styled};
    for mutation_after_paint in [0, 1, 2] {
        let (view, cx) = cx.add_window_view(|w, cx| EditorView::new("abcdefghij".into(), w, cx));
        let result = Rc::new(RefCell::new(None));
        let before = view.clone();
        let after = view.clone();
        let sink = result.clone();
        cx.draw(point(px(0.), px(0.)), size(px(300.), px(100.)), |_, _| {
            gpui::canvas(
                move |bounds, window, cx| {
                    before.update(cx, |ed, cx| {
                        ed.set_text_presentation(Some(presentation(ed.text(), 1, 0..10)), cx)
                            .unwrap();
                        ed.measure_presented_range(
                            1,
                            0..10,
                            10,
                            move |g, _, _| *sink.borrow_mut() = Some(g),
                            cx,
                        )
                        .unwrap();
                    });
                    let bounds = Bounds::new(bounds.origin, size(px(100.), px(20.)));
                    let mask = ContentMask {
                        bounds: Bounds::new(bounds.origin, size(px(15.), px(10.))),
                    };
                    let mut line = super::LineElement {
                        editor: before.clone(),
                        row: 0,
                    };
                    let state = window.with_content_mask(Some(mask.clone()), |window| {
                        line.prepaint(None, None, bounds, &mut (), window, cx)
                    });
                    (line, state, bounds, mask)
                },
                move |_, (mut line, mut state, bounds, mask), window, cx| {
                    window.with_content_mask(Some(mask), |window| {
                        line.paint(None, None, bounds, &mut (), &mut state, window, cx)
                    });
                    match mutation_after_paint {
                        1 => after.update(cx, |ed, _| ed.engine = Editor::new("ABCDEFGHIJ".into())),
                        2 => after.update(cx, |ed, cx| ed.invalidate_presentation_geometry(cx)),
                        _ => {}
                    }
                },
            )
            .size_full()
        });
        cx.run_until_parked();
        if mutation_after_paint != 0 {
            assert!(
                result.borrow().is_none(),
                "same-length revision-zero replacement must suppress receipt"
            );
        } else {
            let g = result
                .borrow()
                .clone()
                .expect("partially visible actual fragment");
            assert!(!g.fully_visible);
            assert!(g.first_visible_fragment.size.width <= px(15.));
            assert!(g.first_visible_fragment.size.height <= px(10.));
        }
    }
}

#[gpui::test]
fn wrapped_multiline_crlf_geometry_uses_drawn_source_fragments(cx: &mut TestAppContext) {
    use gpui::{Bounds, Element, Styled};
    let text = "alpha beta gamma delta\r\n😀 tail";
    let (view, cx) = cx.add_window_view(|w, cx| EditorView::new(text.into(), w, cx));
    let result = Rc::new(RefCell::new(None));
    let before = view.clone();
    let sink = result.clone();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |_, _| {
        gpui::canvas(
            move |bounds, window, cx| {
                before.update(cx, |ed, cx| {
                    ed.set_appearance(crate::EditorAppearance::plain(), cx);
                    ed.viewport_width = px(70.).into();
                    ed.prepare_wrap(window);
                    assert!(ed.display_count() > 2, "fixture must wrap");
                    ed.set_text_presentation(Some(presentation(ed.text(), 9, 0..text.len())), cx)
                        .unwrap();
                    ed.measure_presented_range(
                        9,
                        0..text.len(),
                        90,
                        move |g, _, _| *sink.borrow_mut() = Some(g),
                        cx,
                    )
                    .unwrap();
                });
                let count = before.read(cx).display_count();
                (0..count)
                    .map(|row| {
                        let bounds = Bounds::new(
                            point(bounds.left(), bounds.top() + px(row as f32 * 24.)),
                            size(px(70.), px(24.)),
                        );
                        let mut line = super::LineElement {
                            editor: before.clone(),
                            row,
                        };
                        let state = line.prepaint(None, None, bounds, &mut (), window, cx);
                        (line, state, bounds)
                    })
                    .collect::<Vec<_>>()
            },
            move |_, lines, window, cx| {
                for (mut line, mut state, bounds) in lines {
                    line.paint(None, None, bounds, &mut (), &mut state, window, cx);
                }
            },
        )
        .size_full()
    });
    cx.run_until_parked();
    let g = result.borrow().clone().expect("wrapped receipt");
    assert!(
        g.fully_visible,
        "all wrapped ranges plus CRLF gaps were painted"
    );
    assert_eq!(g.host_generation, 90);
}

#[gpui::test]
fn set_and_clear_redraw_an_otherwise_cached_editor_child(cx: &mut TestAppContext) {
    use gpui::{
        AnyView, AppContext, Context, Entity, ParentElement, Render, StyleRefinement, Styled,
        Window, div,
    };
    struct Host(Entity<EditorView>);
    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().flex().child(
                AnyView::from(self.0.clone())
                    .cached(StyleRefinement::default().flex_1().min_h_0().w_full()),
            )
        }
    }
    let (host, cx) = cx
        .add_window_view(|w, cx| Host(cx.new(|cx| EditorView::new("cached child".into(), w, cx))));
    cx.run_until_parked();
    let child = host.read_with(cx, |h, _| h.0.clone());
    let before = child.read_with(cx, |ed, _| ed.layout_generation);
    host.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        child.read_with(cx, |ed, _| ed.layout_generation),
        before,
        "control: unchanged cached child"
    );
    let installed = child.update(cx, |ed, cx| {
        ed.set_text_presentation(Some(presentation(ed.text(), 1, 0..6)), cx)
            .unwrap();
        ed.layout_generation
    });
    cx.run_until_parked();
    let decorated = child.read_with(cx, |ed, _| ed.layout_generation);
    assert!(
        decorated > installed,
        "set invalidated cached paint, not only metadata"
    );
    let cleared = child.update(cx, |ed, cx| {
        ed.set_text_presentation(None, cx).unwrap();
        ed.layout_generation
    });
    cx.run_until_parked();
    child.read_with(cx, |ed, _| {
        assert!(
            ed.layout_generation > cleared,
            "clear invalidated cached paint"
        );
        assert!(ed.presentation.is_none());
        assert_eq!(ed.text(), "cached child");
        assert_eq!(ed.engine.revision(), 0);
    });
}
