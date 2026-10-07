use super::{Converter, Source};
use crate::image_disposal::DisposalLog;
use gpui::{Bounds, EntityInputHandler, ImageId, TestAppContext, WindowHandle, px, size};

fn reviewed_window(cx: &mut TestAppContext) -> (WindowHandle<Converter>, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let window = cx.add_window(Converter::new);
    window
        .update(cx, |view, _, cx| view.load(Source::Synthetic, cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |view, _, cx| {
            view.model.options.trim_end = Some(0.2);
            view.convert(directory.path().join("preview.gif"), cx);
        })
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| cx.set_global(DisposalLog::default()));
    (window, directory)
}

fn disposed(cx: &TestAppContext) -> Vec<(ImageId, usize)> {
    cx.read(|cx| cx.global::<DisposalLog>().0.clone())
}

#[gpui::test]
fn navigation_retains_previews_but_same_window_rechoose_disposes_both(cx: &mut TestAppContext) {
    let (window, _directory) = reviewed_window(cx);
    let old = window
        .update(cx, |view, _, cx| {
            let old = [
                view.source_preview.image.as_ref().unwrap().id,
                view.preview.image.as_ref().unwrap().id,
            ];
            view.show_movie(cx);
            view.show_gif(cx);
            old
        })
        .unwrap();
    assert!(
        disposed(cx).is_empty(),
        "Movie/GIF navigation retains review"
    );
    window
        .update(cx, |view, _, cx| {
            view.load(Source::Movie("never-opened.mov".into()), cx);
            assert!(view.source_preview.image.is_none());
            assert!(view.preview.image.is_none());
            assert!(cx.global::<DisposalLog>().0.is_empty());
        })
        .unwrap();
    let records = disposed(cx);
    assert_eq!(records.len(), 2);
    for image in old {
        assert!(records.contains(&(image, 1)));
    }
    cx.run_until_parked();
    assert_eq!(disposed(cx), records, "no stale response disposes twice");
}

#[gpui::test]
fn resuming_prefetched_gif_disposes_replaced_frame_after_window_returns(cx: &mut TestAppContext) {
    let (window, _directory) = reviewed_window(cx);
    let old = window
        .update(cx, |view, _, cx| {
            let old = view.preview.image.as_ref().unwrap().id;
            view.toggle_preview(cx);
            view.toggle_preview(cx);
            old
        })
        .unwrap();
    cx.run_until_parked();
    // Decode completed while paused, so Play now presents its prefetched frame
    // synchronously inside the borrowed window's callback.
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.preview.image.as_ref().unwrap().id, old);
            assert!(!view.preview.loading);
            view.toggle_preview(cx);
            assert_ne!(view.preview.image.as_ref().unwrap().id, old);
            assert!(cx.global::<DisposalLog>().0.is_empty());
            view.preview.playing = false;
        })
        .unwrap();
    assert_eq!(disposed(cx), vec![(old, 1)]);
    cx.run_until_parked();
}

#[gpui::test]
fn background_source_completion_disposes_previous_frame_with_live_window(cx: &mut TestAppContext) {
    let (window, _directory) = reviewed_window(cx);
    let old = window
        .update(cx, |view, _, cx| {
            let old = view.source_preview.image.as_ref().unwrap().id;
            view.show_movie(cx);
            view.seek_source(0.642, cx);
            old
        })
        .unwrap();
    assert!(disposed(cx).is_empty());
    cx.run_until_parked();
    assert_eq!(disposed(cx), vec![(old, 1)]);
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.source_preview.completed_request, Some(0.642));
            assert_ne!(view.source_preview.image.as_ref().unwrap().id, old);
        })
        .unwrap();
}

#[gpui::test]
fn retained_close_disposes_images_and_late_seek_cannot_resurrect_them(cx: &mut TestAppContext) {
    let (window, _directory) = reviewed_window(cx);
    let root = window.root(cx).unwrap();
    let old = window
        .update(cx, |view, _, cx| {
            let old = [
                view.source_preview.image.as_ref().unwrap().id,
                view.preview.image.as_ref().unwrap().id,
            ];
            view.seek_source(2., cx);
            view.close(cx);
            assert!(cx.global::<DisposalLog>().0.is_empty());
            old
        })
        .unwrap();
    cx.run_until_parked();
    let records = disposed(cx);
    assert_eq!(records.len(), 2);
    for image in old {
        assert!(records.contains(&(image, 1)));
    }
    root.read_with(cx, |view, _| {
        assert!(view.source_preview.image.is_none());
        assert!(view.preview.image.is_none());
        assert!(view.model.source.is_none());
    });
}

#[gpui::test]
fn removed_window_and_entity_release_do_not_cancel_image_disposal(cx: &mut TestAppContext) {
    for close_explicitly in [true, false] {
        let (window, _directory) = reviewed_window(cx);
        let root = window.root(cx).unwrap();
        let old = window
            .update(cx, |view, window, cx| {
                let old = [
                    view.source_preview.image.as_ref().unwrap().id,
                    view.preview.image.as_ref().unwrap().id,
                ];
                if close_explicitly {
                    view.close(cx);
                }
                window.remove_window();
                old
            })
            .unwrap();
        drop(root);
        cx.update(|_| ());
        cx.run_until_parked();
        let records = disposed(cx);
        assert_eq!(records.len(), 2);
        for image in old {
            assert!(records.contains(&(image, 0)));
        }
    }
}

#[gpui::test]
fn narrow_converter_trim_fields_keep_fraction_prefix_visible_at_end_caret(cx: &mut TestAppContext) {
    let window = cx.add_window(Converter::new);
    cx.simulate_window_resize(window.into(), size(px(560.), px(600.)));
    for text in ["0.642", "120.000", "7200.642"] {
        for is_start in [true, false] {
            window
                .update(cx, |view, window, cx| {
                    let field = if is_start { &view.start } else { &view.end };
                    field.update(cx, |editor, cx| {
                        editor.set_text(String::new(), cx);
                        editor.focus(window);
                        editor.replace_text_in_range(None, text, window, cx);
                    });
                })
                .unwrap();
            cx.run_until_parked();
            // Render again with the measured field width. The compact editor
            // computes its horizontal offset from the previous frame's layout.
            window.update(cx, |_, window, _| window.refresh()).unwrap();
            cx.run_until_parked();
            window
                .update(cx, |view, window, cx| {
                    let field = if is_start { &view.start } else { &view.end };
                    field.update(cx, |editor, cx| {
                        assert_eq!(editor.text(), text, "precision is unchanged");
                        let selection = editor.selected_text_range(false, window, cx).unwrap();
                        assert_eq!(selection.range, text.len()..text.len());
                        let positions: Vec<_> = (0..=text.len())
                            .map(|index| {
                                editor
                                    .bounds_for_range(index..index, Bounds::default(), window, cx)
                                    .unwrap()
                                    .left()
                            })
                            .collect();
                        assert!(
                            positions.windows(2).all(|pair| pair[0] < pair[1]),
                            "every character, including the decimal prefix, is visible: {text}"
                        );
                        assert!(positions[text.len()] - positions[0] < px(110.));
                    });
                })
                .unwrap();
        }
    }
}
