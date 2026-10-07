//! Standalone converter host. Native decoding remains gated; the DEBUG synthetic
//! route exercises this same lifecycle and real bounded Rust GIF export.
mod model;
mod preview;
#[cfg(test)]
mod tests;
use crate::theme::{self, Palette};
use bello_workbench_ui::{EditorAppearance, EditorView};
use bellobox_core::recording::gif::{GifExportOptions, ReplacePolicy};
use gpui::{prelude::*, *};
use model::{Model, Source};
use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};

pub fn open(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(600.), px(630.)), cx);
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(560.), px(600.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Video to GIF — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| Converter::new(window, cx)),
    ) {
        eprintln!("Cannot open GIF converter: {error}");
    }
}
struct Converter {
    model: Model,
    start: Entity<EditorView>,
    end: Entity<EditorView>,
    focus: FocusHandle,
    preview: preview::Preview,
    fields_locked: bool,
    trim_bounds: [std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>; 2],
}
impl Converter {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p = theme::for_window(window);
        let start = editor("0.00", p, window, cx);
        let end = editor("", p, window, cx);
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = weak.update(cx, |this, cx| this.close(cx));
            true
        });
        #[allow(unused_mut)] // DEBUG fixture loads through the ordinary async host.
        let mut this = Self {
            model: Model::default(),
            start,
            end,
            focus: cx.focus_handle(),
            preview: preview::Preview::new(cx),
            fields_locked: false,
            trim_bounds: Default::default(),
        };
        #[cfg(debug_assertions)]
        if std::env::var("BELLOBOX_GIF_FIXTURE").as_deref() == Ok("1") {
            this.load(Source::Synthetic, cx);
        }
        cx.on_release(|this, cx| {
            if let Some(image) = this.preview.image.take() {
                cx.drop_image(image, None);
            }
        })
        .detach();
        this.focus.focus(window);
        this
    }
    fn is_fixture(&self) -> bool {
        #[cfg(any(test, debug_assertions))]
        {
            matches!(self.model.source, Some(Source::Synthetic))
        }
        #[cfg(not(any(test, debug_assertions)))]
        {
            false
        }
    }
    fn close(&mut self, cx: &mut Context<Self>) {
        self.model.close();
        self.preview.reset(cx);
    }
    fn sync_trim(&mut self, cx: &mut Context<Self>) {
        self.start.update(cx, |editor, cx| {
            editor.set_text(self.model.options.trim_start.to_string(), cx)
        });
        self.end.update(cx, |editor, cx| {
            editor.set_text(
                self.model
                    .options
                    .trim_end
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                cx,
            )
        });
    }
    fn apply_trim(&mut self, cx: &mut Context<Self>) -> bool {
        if self.model.busy() {
            return false;
        }
        let start = self.start.read(cx).text().trim().parse::<f64>();
        let end = self.end.read(cx).text().trim().parse::<f64>();
        match (start, end, self.model.info) {
            (Ok(start), Ok(end), Some(info))
                if start.is_finite()
                    && end.is_finite()
                    && start >= 0.
                    && end <= info.duration.max(0.1)
                    && end - start >= 0.05
                    && end - start <= 120.000_001 =>
            {
                self.model.options.trim_start = start;
                self.model.options.trim_end = Some(end);
                self.model.status = "Trim applied.".into();
                self.model.error = false;
                cx.notify();
                true
            }
            _ => {
                self.model.status =
                    "Enter a valid start and end within the movie, from 0.05 to 120 seconds apart."
                        .into();
                self.model.error = true;
                cx.notify();
                false
            }
        }
    }
    fn load(&mut self, source: Source, cx: &mut Context<Self>) {
        let Some((generation, cancellation)) = self.model.load(source.clone()) else {
            return;
        };
        self.preview.reset(cx);
        self.sync_trim(cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { source.inspect(cancellation) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.model.generation == generation {
                    this.model.loaded(generation, result);
                    this.sync_trim(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn accepts_choose_shortcut(&self, event: &KeyDownEvent) -> bool {
        event.keystroke.key == "o"
            && event.keystroke.modifiers.platform
            && self.model.source.is_none()
            && !self.model.busy()
    }
    fn choose(&mut self, cx: &mut Context<Self>) {
        if self.model.busy() {
            return;
        }
        self.model.dialog = true;
        let generation = self.model.generation;
        let dialog = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a local MOV or MP4 movie".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = dialog.await;
            let _ = this.update(cx, |this, cx| {
                if this.model.generation != generation {
                    return;
                }
                this.model.dialog = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.load(Source::Movie(path), cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        this.model.status = "Could not open the movie chooser.".into();
                        this.model.error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.model.busy() || !self.apply_trim(cx) || self.model.plan().is_none() {
            return;
        }
        self.model.dialog = true;
        let generation = self.model.generation;
        let name = match &self.model.source {
            Some(Source::Movie(path)) => format!(
                "{}.gif",
                path.file_stem().unwrap_or_default().to_string_lossy()
            ),
            _ => "BelloBox-synthetic.gif".into(),
        };
        let dialog = cx.prompt_for_new_path(
            &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            Some(&name),
        );
        cx.spawn(async move |this, cx| {
            let result = dialog.await;
            let _ = this.update(cx, |this, cx| {
                if this.model.generation != generation {
                    return;
                }
                this.model.dialog = false;
                match result {
                    // GPUI does not expose overwrite confirmation evidence. Preserve
                    // existing files instead of inferring authorization from a path.
                    Ok(Ok(Some(path))) => this.convert(path, cx),
                    Ok(Ok(None)) => {}
                    _ => {
                        this.model.status = "Could not open the GIF save dialog.".into();
                        this.model.error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn convert(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
        {
            self.model.status = "Choose a filename ending in .gif.".into();
            self.model.error = true;
            cx.notify();
            return;
        }
        let Some(work) = self.model.begin(path, ReplacePolicy::RefuseExisting) else {
            return;
        };
        self.preview.reset(cx);
        let generation = work.generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { work.run() })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.model.generation != generation {
                    return;
                }
                this.model.finished(generation, result);
                if this.model.result.is_some() {
                    this.preview_result(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                if !this
                    .update(cx, |this, cx| {
                        let running =
                            this.model.generation == generation && this.model.job.is_some();
                        if running {
                            cx.notify();
                        }
                        running
                    })
                    .unwrap_or(false)
                {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    #[cfg(debug_assertions)]
    fn fixture_export(&mut self, cx: &mut Context<Self>) {
        if self.model.busy() || !self.apply_trim(cx) {
            return;
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "BelloBox-synthetic-{}-{nonce}.gif",
            std::process::id()
        ));
        self.convert(path, cx);
    }
}
impl Converter {
    fn move_trim(&mut self, start: bool, x: Pixels, cx: &mut Context<Self>) {
        if self.model.busy() {
            return;
        }
        let Some(info) = self.model.info else {
            return;
        };
        let bounds = self.trim_bounds[usize::from(!start)].get();
        let fraction = (f32::from(x - bounds.origin.x - px(4.))
            / (f32::from(bounds.size.width) - 8.).max(1.))
        .clamp(0., 1.);
        model::move_trim(
            &mut self.model.options,
            info.duration,
            start,
            f64::from(fraction) * info.duration,
        );
        self.sync_trim(cx);
        cx.notify();
    }
    fn trim_slider(&self, start: bool, p: Palette, cx: &mut Context<Self>) -> Div {
        let bounds = self.trim_bounds[usize::from(!start)].clone();
        let duration = self.model.info.map_or(1., |info| info.duration.max(0.1));
        let value = if start {
            self.model.options.trim_start
        } else {
            self.model.options.trim_end.unwrap_or(duration)
        };
        let fraction = (value / duration).clamp(0., 1.) as f32;
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().w(px(48.)).child(if start { "Start" } else { "End" }))
            .child(
                div()
                    .id(if start {
                        "trim-start-slider"
                    } else {
                        "trim-end-slider"
                    })
                    .h(px(20.))
                    .flex_1()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.move_trim(start, event.position.x, cx)
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        if event.pressed_button == Some(MouseButton::Left) {
                            this.move_trim(start, event.position.x, cx);
                        }
                    }))
                    .child(
                        canvas(
                            move |b, _, _| bounds.set(b),
                            move |b, _, window, _| {
                                let width = (f32::from(b.size.width) - 8.).max(1.);
                                let left = b.origin.x + px(4.);
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(left, b.origin.y + px(9.)),
                                        size(px(width), px(3.)),
                                    ),
                                    p.border,
                                ));
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(
                                            left + px(width * fraction) - px(4.),
                                            b.origin.y + px(5.),
                                        ),
                                        size(px(9.), px(11.)),
                                    ),
                                    p.accent,
                                ));
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}
impl Render for Converter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        let fixture = self.is_fixture();
        let busy = self.model.busy();
        if self.fields_locked != busy {
            self.fields_locked = busy;
            self.start
                .update(cx, |editor, cx| editor.set_read_only(busy, cx));
            self.end
                .update(cx, |editor, cx| editor.set_read_only(busy, cx));
        }
        let plan = self.model.plan();
        let source = self
            .model
            .source
            .as_ref()
            .map(Source::name)
            .unwrap_or_else(|| "Choose a local movie to convert".into());
        let mut controls = div().flex().flex_col().gap_3();
        for (label, values, current, fps) in [
            (
                "Frame rate",
                GifExportOptions::FRAME_RATE_CHOICES.to_vec(),
                self.model.options.frames_per_second,
                true,
            ),
            (
                "Longest edge",
                GifExportOptions::WIDTH_CHOICES.to_vec(),
                self.model.options.max_width,
                false,
            ),
        ] {
            controls = controls.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(105.)).child(label))
                    .children(values.into_iter().map(|value| {
                        button(
                            (if fps { "fps" } else { "width" }, value as u64),
                            value.to_string(),
                            p,
                        )
                        .when(value == current, |el| {
                            el.bg(p.accent_fill).text_color(rgb(0xffffff))
                        })
                        .when(!busy, |el| {
                            el.on_click(cx.listener(move |this, _, _, cx| {
                                if fps {
                                    this.model.options.frames_per_second = value;
                                } else {
                                    this.model.options.max_width = value;
                                }
                                cx.notify();
                            }))
                        })
                    })),
            );
        }
        let summary = plan
            .as_ref()
            .map(|plan| {
                format!(
                    "{} frames · {} × {} px · {:.2} s",
                    plan.frame_count(),
                    plan.output_size().0,
                    plan.output_size().1,
                    plan.encoded_duration()
                )
            })
            .unwrap_or_else(|| "Choose a supported source to plan the GIF.".into());
        div().size_full().bg(p.bg).text_color(p.primary).font_family(theme::ui_font()).text_sm().p_4().id("gif-converter").overflow_y_scroll().track_focus(&self.focus)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.accepts_choose_shortcut(event) {
                    this.choose(cx);
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.key == "tab" && !this.start.read(cx).has_marked_text() && !this.end.read(cx).has_marked_text() {
                    let start = this.start.read(cx).focus_handle(cx);
                    let end = this.end.read(cx).focus_handle(cx);
                    if !event.keystroke.modifiers.shift && start.is_focused(window) { end.focus(window); }
                    else if event.keystroke.modifiers.shift && end.is_focused(window) { start.focus(window); }
                    else if event.keystroke.modifiers.shift { window.focus_prev(); } else { window.focus_next(); }
                    cx.stop_propagation();
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| { if event.keystroke.key == "escape" && !this.start.read(cx).has_marked_text() && !this.end.read(cx).has_marked_text() { this.close(cx); window.remove_window(); } }))
            .child(div().flex().flex_col().gap_3()
                .child(div().text_xl().child("Video to GIF"))
                .child(div().text_color(p.secondary).child(source))
                .child(div().min_h(px(120.)).max_h(px(180.)).p_2().rounded_lg().bg(p.well).flex().flex_col().justify_center().gap_2()
                    .when_some(self.preview.image.clone(), |el, image| el.child(img(image).h(px(140.)).object_fit(ObjectFit::Contain)))
                    .when(self.preview.image.is_none(), |el| el.child(if fixture { "Synthetic color movie · 320 × 180 · 3 seconds" } else { "Native movie decoding is unavailable in this Rust preview." }))
                    .child(div().text_xs().text_color(p.secondary).child(if self.preview.image.is_some() { "Exported GIF preview · starts paused" } else { "Native movie playback and scrubbing are not implemented." })))
                .when(self.preview.image.is_some(), |el| el.child(button("preview-play", if self.preview.playing { "Pause GIF" } else { "Play GIF" }, p).when(!busy, |el| el.on_click(cx.listener(|this, _, _, cx| this.toggle_preview(cx))))))
                .child(button("choose", if self.model.source.is_some() { "Choose Another…" } else { "Choose Movie…" }, p).when(!busy, |el| el.on_click(cx.listener(|this, _, _, cx| this.choose(cx)))))
                .child(controls)
                .child(button("loop", if self.model.options.loops { "Loop: On" } else { "Loop: Once" }, p).when(!busy, |el| el.on_click(cx.listener(|this, _, _, cx| { this.model.options.loops = !this.model.options.loops; cx.notify(); }))))
                .child(self.trim_slider(true, p, cx))
                .child(self.trim_slider(false, p, cx))
                .child(div().flex().items_center().gap_2().child("Trim (seconds)").child(field(self.start.clone(), p)).child("to").child(field(self.end.clone(), p)).child(button("trim", "Apply", p).when(!busy, |el| el.on_click(cx.listener(|this, _, _, cx| { this.apply_trim(cx); })))))
                .child(div().child(summary).child(div().text_color(p.secondary).child("Silent · file size shown after writing · maximum 120 seconds")))
                .child(div().flex().items_center().gap_2()
                    .child(button("convert", "Convert to GIF…", p).when(!busy && plan.is_some(), |el| el.on_click(cx.listener(|this, _, _, cx| this.save(cx)))))
                    .when(self.model.job.is_some(), |el| el.child(button("cancel", "Cancel", p).on_click(cx.listener(|this, _, _, cx| { this.model.cancel(); cx.notify(); }))))
                    .when_some(self.model.job.as_ref(), |el, job| el.child(format!("Encoding {}%", job.progress.load(Ordering::Acquire) * 100 / job.planned.max(1)))))
                .when(fixture, |el| {
                    #[cfg(debug_assertions)]
                    let el = el.child(button("fixture-export", "Export synthetic GIF to temporary folder", p).when(!busy && plan.is_some(), |el| el.on_click(cx.listener(|this, _, _, cx| this.fixture_export(cx)))));
                    el
                })
                .when_some(self.model.result.as_ref(), |el, result| el.child(div().flex().flex_col().gap_2().child(format!("Saved: {} frames · {} × {} · {} bytes", result.frame_count, result.size.0, result.size.1, result.file_size)).child(button("reveal", "Show GIF in folder", p).on_click(cx.listener(|this, _, _, cx| { if let Some(result) = &this.model.result { cx.reveal_path(&result.path); } }))).child(button("copy-path", "Copy GIF path", p).on_click(cx.listener(|this, _, _, cx| { if let Some(result) = &this.model.result { cx.write_to_clipboard(ClipboardItem::new_string(result.path.to_string_lossy().into_owned())); this.model.status = "Copied GIF path (file clipboard is not supported).".into(); cx.notify(); } })))))
                .child(div().text_color(if self.model.error { p.danger } else { p.secondary }).child(self.model.status.clone()))
                .child(div().text_xs().text_color(p.secondary).child("Converted locally. Nothing is uploaded. Existing destination files are kept.")))
    }
}
fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .focusable()
        .tab_index(0)
        .focus(|style| style.border_color(p.accent))
        .px_3()
        .py_1()
        .rounded_md()
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .cursor_pointer()
        .child(label.into())
}
fn editor(
    text: &str,
    p: Palette,
    window: &mut Window,
    cx: &mut Context<Converter>,
) -> Entity<EditorView> {
    cx.new(|cx| {
        let mut editor = EditorView::new(text.into(), window, cx);
        let mut appearance = EditorAppearance::plain();
        appearance.font_size = 13.;
        appearance.line_height = 22.;
        appearance.padding_x = 0.;
        appearance.padding_y = 0.;
        appearance.text = p.primary;
        appearance.caret = p.accent;
        editor.set_appearance(appearance, cx);
        editor.set_compact(true, cx);
        editor
    })
}
fn field(editor: Entity<EditorView>, p: Palette) -> Div {
    div()
        .w(px(70.))
        .h(px(28.))
        .px_1()
        .bg(p.well)
        .border_1()
        .border_color(p.border)
        .rounded_md()
        .child(editor)
}
