use super::*;
use gpui::{
    EntityInputHandler, Focusable, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext,
    VisualTestContext,
};

#[gpui::test]
fn popup_refused_copy_preserves_sentinel_and_save_stays_available(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "current".into(), w, cx));
    cx.run_until_parked();
    view.update(cx, |v, _, cx| {
        assert_eq!(v.qr_copy_notice, crate::qr_clipboard::for_app(cx));
        assert!(v.qr.is_some());
        for backend in ["X11", "Wayland", "headless", "", "unknown compositor"] {
            v.qr_copy_notice = crate::qr_clipboard::image_copy_notice(backend, false);
            assert!(!v.can_copy_qr());
            cx.write_to_clipboard(ClipboardItem::new_string("external sentinel".into()));
            v.copy_qr(cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some("external sentinel")
            );
            assert_eq!(v.status, v.qr_copy_notice.unwrap());
            assert!(v.status.contains("Save…"));
        }
        v.save_qr(cx);
        assert!(v.qr_save_status);
    })
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("supported-alternative.png");
    cx.simulate_new_path_selection(|_| Some(output.clone()));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(output).unwrap(),
        bellobox_core::qr::png("current").unwrap()
    );
}

#[gpui::test]
fn popup_synthetic_image_fixture_copies_only_current_generation(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "current".into(), w, cx));
    cx.run_until_parked();
    view.update(cx, |v, _, cx| {
        // Explicit in-memory fixture, never a capability inferred from TestPlatform.
        v.qr_copy_notice = None;
        assert!(v.can_copy_qr());
        v.copy_qr(cx);
        let item = cx.read_from_clipboard().unwrap();
        let gpui::ClipboardEntry::Image(image) = &item.entries()[0] else {
            panic!("expected image");
        };
        assert_eq!(image.bytes(), bellobox_core::qr::png("current").unwrap());
        for invalid in ["".to_string(), " \n\t".into(), "x".repeat(2001)] {
            v.input.update(cx, |e, cx| e.set_text(invalid, cx));
            v.run_tool(cx);
            assert!(!v.can_copy_qr());
            cx.write_to_clipboard(ClipboardItem::new_string("keep invalid".into()));
            v.copy_qr(cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some("keep invalid")
            );
            assert!(!v.status.contains("Copied"));
        }
    })
    .unwrap();
}

#[gpui::test]
fn popup_disabled_copy_pointer_never_writes_or_claims_success(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "current".into(), w, cx));
    cx.run_until_parked();
    view.update(cx, |v, _, cx| {
        v.qr_copy_notice = crate::qr_clipboard::image_copy_notice("X11", false);
        cx.write_to_clipboard(ClipboardItem::new_string("pointer sentinel".into()));
        cx.notify();
    })
    .unwrap();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    let point = visual.debug_bounds("qr-copy").unwrap().center();
    visual.simulate_click(point, gpui::Modifiers::default());
    view.update(cx, |v, _, cx| {
        assert!(!v.status.contains("Copied"));
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("pointer sentinel")
        );
    })
    .unwrap();
}

#[gpui::test]
fn popup_editor_activation_keys_block_pointer_save_without_consuming_input_or_replay(
    cx: &mut TestAppContext,
) {
    for composing in [false, true] {
        for key in ["enter", "space"] {
            let view = cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "draft".into(), w, cx));
            cx.run_until_parked();
            view.update(cx, |v, w, cx| {
                v.input.read(cx).focus(w);
                if composing {
                    v.input.update(cx, |e, cx| {
                        e.replace_and_mark_text_in_range(Some(0..2), "qr", Some(2..2), w, cx)
                    });
                }
            })
            .unwrap();
            cx.run_until_parked();
            let mut visual = VisualTestContext::from_window(view.into(), cx);
            if composing {
                visual.simulate_event(KeyDownEvent {
                    keystroke: Keystroke::parse(key).unwrap(),
                    is_held: false,
                });
            } else {
                cx.simulate_keystrokes(view.into(), key);
                view.update(cx, |v, _, cx| {
                    let expected = if key == "enter" { '\n' } else { ' ' };
                    assert!(v.input.read(cx).text().contains(expected));
                })
                .unwrap();
            }
            // Repeat remains evidence; it cannot open the dialog either.
            visual.simulate_event(KeyDownEvent {
                keystroke: Keystroke::parse(key).unwrap(),
                is_held: true,
            });
            cx.run_until_parked();
            view.update(cx, |v, w, cx| {
                assert!(v.qr_activation_keys.any_down());
                assert!(v.input.read(cx).focus_handle(cx).is_focused(w));
                if composing {
                    assert!(v.input.read(cx).has_marked_text());
                }
            })
            .unwrap();
            let save = visual.debug_bounds("qr-save").unwrap().center();
            visual.simulate_click(save, gpui::Modifiers::default());
            view.update(cx, |v, _, _| {
                assert!(!v.qr_save_status);
                assert!(v.status.contains("Release Enter/Space"));
            })
            .unwrap();
            visual.simulate_event(KeyUpEvent {
                keystroke: Keystroke::parse(key).unwrap(),
            });
            view.update(cx, |v, w, cx| {
                assert!(!v.qr_activation_keys.any_down());
                assert!(!v.qr_save_status);
                if composing {
                    v.input.update(cx, |e, cx| e.unmark_text(w, cx));
                }
            })
            .unwrap();
            cx.run_until_parked();
            visual.simulate_click(save, gpui::Modifiers::default());
            view.update(cx, |v, _, _| assert!(v.qr_save_status))
                .unwrap();
            cx.simulate_new_path_selection(|_| None);
            cx.run_until_parked();
            view.update(cx, |_, w, _| w.remove_window()).unwrap();
        }
    }
}

#[gpui::test]
fn popup_save_and_copy_remain_inside_source_sized_windows(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "constrained".into(), w, cx));
    cx.run_until_parked();
    for (width, height) in [(520., 620.), (400., 480.)] {
        cx.simulate_window_resize(view.into(), size(px(width), px(height)));
        for status in [
            "Save cancelled.".to_string(),
            format!(
                "Not saved: /{} exists. Existing files are never overwritten.",
                "very-long-parent-directory/".repeat(12)
            ),
        ] {
            view.update(cx, |v, _, cx| {
                v.qr_copy_notice = crate::qr_clipboard::image_copy_notice("X11", false);
                v.status = status;
                cx.notify();
            })
            .unwrap();
            cx.run_until_parked();
            let mut visual = VisualTestContext::from_window(view.into(), cx);
            for selector in ["qr-save", "qr-copy", "qr-status"] {
                let bounds = visual.debug_bounds(selector).unwrap();
                assert!(bounds.left() >= px(0.), "{selector}: {bounds:?}");
                assert!(bounds.right() <= px(width), "{selector}: {bounds:?}");
                assert!(bounds.top() >= px(0.), "{selector}: {bounds:?}");
                assert!(bounds.bottom() <= px(height), "{selector}: {bounds:?}");
            }
        }
    }
}

#[gpui::test]
fn popup_activation_evidence_is_per_window_and_requires_both_releases(cx: &mut TestAppContext) {
    let first = cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "first".into(), w, cx));
    let second = cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "second".into(), w, cx));
    cx.run_until_parked();
    first
        .update(cx, |v, w, cx| v.input.read(cx).focus(w))
        .unwrap();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(first.into(), cx);
    for key in ["enter", "space"] {
        visual.simulate_event(KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
        });
    }
    cx.run_until_parked();
    first
        .update(cx, |v, _, cx| {
            v.save_qr(cx);
            assert!(!v.qr_save_status);
        })
        .unwrap();
    second
        .update(cx, |v, _, _| assert!(!v.qr_activation_keys.any_down()))
        .unwrap();
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    first
        .update(cx, |v, _, cx| {
            v.save_qr(cx);
            assert!(!v.qr_save_status);
            assert!(v.qr_activation_keys.any_down());
        })
        .unwrap();
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    first
        .update(cx, |v, _, _| {
            assert!(!v.qr_save_status);
            assert!(!v.qr_activation_keys.any_down());
        })
        .unwrap();
    // Closing a window never transfers its held-key evidence to another owner.
    visual.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: false,
    });
    first
        .update(cx, |v, _, _| assert!(v.qr_activation_keys.any_down()))
        .unwrap();
    first.update(cx, |_, w, _| w.remove_window()).unwrap();
    second.update(cx, |v, _, cx| v.save_qr(cx)).unwrap();
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    second
        .update(cx, |v, _, _| assert_eq!(v.status, "Save cancelled."))
        .unwrap();
}

#[gpui::test]
fn popup_refused_save_focus_supports_missed_release_recovery_without_editing(
    cx: &mut TestAppContext,
) {
    for key in ["enter", "space"] {
        let view = cx.add_window(|w, cx| {
            BelloBox::new_for("qr".into(), "draft must stay exact".into(), w, cx)
        });
        cx.run_until_parked();
        view.update(cx, |v, w, cx| v.input.read(cx).focus(w))
            .unwrap();
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(view.into(), cx);
        visual.simulate_event(KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
        });
        cx.run_until_parked();
        cx.simulate_keystrokes(
            view.into(),
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
        );
        let selection = view
            .update(cx, |v, w, cx| {
                v.input.update(cx, |e, cx| {
                    e.selected_text_range(false, w, cx).unwrap().range
                })
            })
            .unwrap();
        assert!(!selection.is_empty());
        // Treat the old key-up as missed. The Save click must not erase evidence.
        let save = visual.debug_bounds("qr-save").unwrap().center();
        visual.simulate_click(save, gpui::Modifiers::default());
        let snapshot = view
            .update(cx, |v, w, cx| {
                assert!(!v.qr_save_status);
                assert!(v.qr_activation_keys.any_down());
                assert!(v.qr_save_focus.is_focused(w));
                v.input.read(cx).text().to_owned()
            })
            .unwrap();
        assert!(!cx.did_prompt_for_new_path());
        cx.simulate_keystrokes(view.into(), key);
        visual.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
        view.update(cx, |v, w, cx| {
            assert_eq!(v.input.read(cx).text(), snapshot);
            let after = v.input.update(cx, |e, cx| {
                e.selected_text_range(false, w, cx).unwrap().range
            });
            assert_eq!(after, selection);
            assert!(!v.qr_activation_keys.any_down());
            assert!(!v.qr_save_status);
        })
        .unwrap();
        assert!(!cx.did_prompt_for_new_path());
        visual.simulate_click(save, gpui::Modifiers::default());
        assert!(cx.did_prompt_for_new_path());
        view.update(cx, |v, _, _| assert!(v.qr_save_status))
            .unwrap();
        cx.simulate_new_path_selection(|_| None);
        assert!(
            !cx.did_prompt_for_new_path(),
            "fresh pointer Save opens exactly one chooser"
        );
        cx.run_until_parked();
        view.update(cx, |_, w, _| w.remove_window()).unwrap();
    }
}

#[gpui::test]
fn popup_qr_preview_fits_without_overlapping_editor_at_supported_sizes(cx: &mut TestAppContext) {
    let view =
        cx.add_window(|w, cx| BelloBox::new_for("qr".into(), "bounded preview".into(), w, cx));
    cx.run_until_parked();
    for (width, height) in [
        (520., 620.),
        (400., 480.),
        (520., 700.),
        (400., 480.),
        (520., 620.),
    ] {
        cx.simulate_window_resize(view.into(), size(px(width), px(height)));
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(view.into(), cx);
        let preview = visual.debug_bounds("qr-preview").unwrap();
        let image = visual.debug_bounds("qr-image").unwrap();
        let editor = visual.debug_bounds("qr-editor").unwrap();
        let pixels = visual.debug_bounds("qr-image-pixels").unwrap();
        assert_eq!(image.size.width, image.size.height);
        assert_eq!(pixels.size.width, pixels.size.height);
        assert_eq!(pixels.size.width, image.size.width - px(28.));
        assert!(pixels.top() >= image.top() && pixels.bottom() <= image.bottom());
        assert!(pixels.left() >= image.left() && pixels.right() <= image.right());
        eprintln!("{width}x{height}: preview={preview:?}, image={image:?}, editor={editor:?}");
        assert!(image.left() >= preview.left() && image.right() <= preview.right());
        assert!(image.top() >= preview.top() && image.bottom() <= preview.bottom());
        assert!(image.bottom() <= editor.top());
        assert!(image.size.width <= px(296.) && image.size.height <= px(296.));
        if height == 700. {
            assert_eq!(image.size, size(px(296.), px(296.)));
        }
    }
}

struct DecodedQrImage(gpui::ImageSource, Option<f32>);
impl Render for DecodedQrImage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .when_some(self.1, |host, side| host.size(px(side)))
            .child(qr_image_element(self.0.clone()))
    }
}

#[gpui::test]
fn popup_decoded_intrinsic_image_uses_exact_final_square_bounds(cx: &mut TestAppContext) {
    let view = cx.add_window(|_, cx| {
        let bytes = bellobox_core::qr::png("decoded PNG intrinsic dimensions").unwrap();
        let image = Image::from_bytes(gpui::ImageFormat::Png, bytes);
        let decoded = image.to_image_data(cx.svg_renderer()).unwrap();
        assert!(decoded.size(0).width > gpui::DevicePixels(296));
        DecodedQrImage(decoded.into(), None)
    });
    for (width, height) in [
        (496., 279.5),
        (376., 161.5),
        (496., 300.),
        (376., 161.5),
        (0., 0.),
        (20., 20.),
        (28., 28.),
        (496., 279.5),
    ] {
        cx.simulate_window_resize(view.into(), size(px(width), px(height)));
        cx.run_until_parked();
        for _ in 0..2 {
            view.update(cx, |_, w, _| w.refresh()).unwrap();
            cx.run_until_parked();
            let mut visual = VisualTestContext::from_window(view.into(), cx);
            if width.min(height) <= 28. {
                // GPUI 0.2.2 retains debug-bound entries across frames. Absence
                // is checked on separate fresh tiny hosts below, not this map.
                continue;
            }
            let image = visual.debug_bounds("qr-image").unwrap();
            let pixels = visual.debug_bounds("qr-image-pixels").unwrap();
            let side = px(width.min(height).min(296.));
            assert_eq!(image.size, size(side, side));
            assert_eq!(pixels.size, size(side - px(28.), side - px(28.)));
            assert!(pixels.left() >= image.left() && pixels.right() <= image.right());
            assert!(pixels.top() >= image.top() && pixels.bottom() <= image.bottom());
            assert!(image.top() >= px(0.) && image.bottom() <= px(height));
            cx.run_until_parked();
        }
    }
}

#[gpui::test]
fn popup_tiny_image_hosts_do_not_create_overflowing_padded_children(cx: &mut TestAppContext) {
    for side in [0., 20., 28.] {
        let view = cx.add_window(|_, cx| {
            let image = Image::from_bytes(
                gpui::ImageFormat::Png,
                bellobox_core::qr::png("tiny host").unwrap(),
            );
            DecodedQrImage(
                image.to_image_data(cx.svg_renderer()).unwrap().into(),
                Some(side),
            )
        });
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(view.into(), cx);
        assert!(visual.debug_bounds("qr-image").is_none());
        assert!(visual.debug_bounds("qr-image-pixels").is_none());
        view.update(cx, |_, w, _| w.remove_window()).unwrap();
    }
}
