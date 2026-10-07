//! The source capture overlay is a host for the real ScreenshotEditor, not a
//! second document or export implementation. Full-display preview tiles stay in
//! document coordinates while a draft crop moves above the frozen background.
use super::*;
use bello_platform::macos_capture_overlay::MainDisplayOverlayLayout;
use bellobox_core::screenshot::selection::{self, SelectionAdjustmentDraft, SelectionHandle};

pub(super) struct InlineArea {
    pub id: u64,
    pub layout: MainDisplayOverlayLayout,
    pub adjustment: Option<Adjustment>,
    menu_anchor: Option<ViewPoint<Pixels>>,
}
pub(super) struct Adjustment {
    draft: SelectionAdjustmentDraft,
    start: Point,
    start_rect: Rect,
    handle: Option<SelectionHandle>,
}
impl InlineArea {
    pub fn new(id: u64, layout: MainDisplayOverlayLayout) -> Self {
        Self {
            id,
            layout,
            adjustment: None,
            menu_anchor: None,
        }
    }
    fn bounds(&self) -> Rect {
        Rect::new(
            0.,
            0.,
            self.layout.cocoa_frame.size.width as f32,
            self.layout.cocoa_frame.size.height as f32,
        )
    }
    pub fn pixels_per_point(&self) -> (f32, f32) {
        (
            self.layout.display.pixels.width as f32 / self.bounds().width,
            self.layout.display.pixels.height as f32 / self.bounds().height,
        )
    }
    pub fn scales(&self) -> (f32, f32) {
        let (x, y) = self.pixels_per_point();
        (1. / x, 1. / y)
    }
    fn local_rect(&self, pixels: Rect) -> Rect {
        let (x, y) = self.pixels_per_point();
        Rect::new(
            pixels.x / x,
            pixels.y / y,
            pixels.width / x,
            pixels.height / y,
        )
    }
}
/// CaptureOverlayAccessoryLayout.frame, including its asymmetric fallback and
/// source 10-point gap, 12-point inset and 44-point toolbar height.
pub(super) fn toolbar_frame(selection: Rect, bounds: Rect) -> Rect {
    let width = 980_f32.min((bounds.width - 24.).max(280.));
    // The source's min(preferredHeight, max(52, available)) is always 44 here.
    let height = 44.;
    let x = selection.x.max(12.).min(bounds.right() - width - 12.);
    let mut y = selection.y - height - 10.;
    if y < bounds.y + 12. {
        y = selection.bottom() + 10.;
    }
    y = y.max(bounds.y + 12.).min(bounds.bottom() - height - 12.);
    Rect::new(x, y, width, height)
}
impl ScreenshotEditor {
    pub(super) fn inline_visible_rect(&self) -> Rect {
        self.inline
            .as_ref()
            .and_then(|inline| inline.adjustment.as_ref())
            .map_or_else(
                || self.session.document().visible_rect(),
                |drag| drag.draft.preview_rect(),
            )
    }
    pub(super) fn record_inline_menu_anchor(&mut self, point: ViewPoint<Pixels>) {
        if let Some(inline) = self.inline.as_mut() {
            inline.menu_anchor = Some(point);
        }
    }
    pub(super) fn inline_panel_origin(
        &self,
        width: f32,
        height: f32,
        label: bool,
    ) -> Option<Point> {
        let inline = self.inline.as_ref()?;
        let bounds = inline.bounds();
        let toolbar = toolbar_frame(self.inline_selection(), bounds);
        let anchor = if label {
            self.label_menu_position
        } else {
            inline
                .menu_anchor
                .unwrap_or(point(px(toolbar.x), px(toolbar.bottom())))
        };
        let x = f32::from(anchor.x).clamp(12., (bounds.right() - width - 12.).max(12.));
        let mut y = if label {
            f32::from(anchor.y)
        } else {
            toolbar.bottom() + 6.
        };
        if y + height > bounds.bottom() - 12. {
            y = toolbar.y - height - 6.;
        }
        Some(Point::new(
            x,
            y.clamp(12., (bounds.bottom() - height - 12.).max(12.)),
        ))
    }
    pub(super) fn inline_selection(&self) -> Rect {
        self.inline
            .as_ref()
            .expect("inline host")
            .local_rect(self.inline_visible_rect())
    }
    pub(super) fn begin_selection_adjustment(
        &mut self,
        handle: Option<SelectionHandle>,
        position: ViewPoint<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return;
        }
        if self.export_busy
            || self.show_discard
            || self.open_menu.is_some()
            || self.color_target.is_some()
            || self.inline.as_ref().is_none_or(|i| i.adjustment.is_some())
        {
            return;
        }
        if !self.commit_text(cx) {
            return;
        }
        self.finish_label_drag(cx);
        let inline = self.inline.as_mut().expect("inline host");
        let Ok(Some(draft)) = SelectionAdjustmentDraft::begin(
            &self.session,
            true,
            inline.layout.backing_scale as f32,
        ) else {
            return;
        };
        let start_rect = inline.local_rect(draft.preview_rect());
        inline.adjustment = Some(Adjustment {
            draft,
            start: Point::new(f32::from(position.x), f32::from(position.y)),
            start_rect,
            handle,
        });
        self.gesture = None;
        // Pending OCR is no longer valid once the user starts changing its crop.
        self.ocr_jobs.cancel();
        self.ocr_busy = false;
        self.ocr_content.invalidate();
        self.ocr.update(cx, |e, cx| e.set_text(String::new(), cx));
        cx.notify();
    }
    pub(super) fn update_selection_adjustment(
        &mut self,
        position: ViewPoint<Pixels>,
        finish: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return true;
        }
        let Some(inline) = self.inline.as_mut() else {
            return false;
        };
        let bounds = inline.bounds();
        let (sx, sy) = inline.pixels_per_point();
        let Some(drag) = inline.adjustment.as_mut() else {
            return false;
        };
        let translation = Point::new(
            f32::from(position.x) - drag.start.x,
            f32::from(position.y) - drag.start.y,
        );
        let rect = if let Some(handle) = drag.handle {
            selection::resized_rect(drag.start_rect, handle, translation, bounds, 8.)
        } else {
            selection::moved_rect(drag.start_rect, translation, bounds)
        };
        let result = rect.and_then(|r| {
            drag.draft
                .update(Rect::new(r.x * sx, r.y * sy, r.width * sx, r.height * sy))
        });
        if let Err(error) = result {
            inline.adjustment = None;
            self.status = error;
            self.error = true;
        } else if finish {
            let drag = inline.adjustment.take().expect("active adjustment");
            match drag.draft.commit(&mut self.session) {
                Ok(true) => self.changed(cx),
                Ok(false) => {}
                Err(error) => {
                    self.status = error;
                    self.error = true;
                }
            }
        }
        cx.notify();
        true
    }
    pub(super) fn commit_selection_adjustment(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self
            .inline
            .as_mut()
            .and_then(|inline| inline.adjustment.take())
        else {
            return true;
        };
        match drag.draft.commit(&mut self.session) {
            Ok(true) => {
                self.changed(cx);
                true
            }
            Ok(false) => {
                cx.notify();
                true
            }
            Err(error) => {
                self.status = error;
                self.error = true;
                cx.notify();
                false
            }
        }
    }
    pub(super) fn cancel_inline_owner(&self, cx: &mut App) {
        if let Some(inline) = self.inline.as_ref() {
            main_area::cancel(inline.id, cx);
        }
    }
    pub(super) fn retire_inline(&mut self, cx: &mut App) {
        self.jobs.cancel();
        self.ocr_jobs.cancel();
        self.cancel_window_refresh();
        self.gesture = None;
        self.label_drag = None;
        if let Some(inline) = self.inline.as_mut() {
            inline.adjustment = None;
            // Leave only an inert transparent pixel in a retained UI entity.
            // Last full RGBA/history/PNG owners are released by counted worker
            // disposal, even when callers keep this editor Entity after Close.
            const RETIRED_PIXEL: &[u8] = &[
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96,
                0, 2, 0, 0, 5, 0, 1, 122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
                130,
            ];
            let empty =
                ScreenshotDocument::from_png(RETIRED_PIXEL).expect("built-in transparent pixel");
            let session = std::mem::replace(&mut self.session, ScreenshotEditSession::new(empty));
            let tiles = std::mem::take(&mut self.preview_tiles);
            main_area::discard(inline.id, (session, tiles), cx);
        }
    }
    pub(super) fn render_inline(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let p = theme::for_window(window);
        let inline = self.inline.as_ref().expect("inline host");
        let bounds = inline.bounds();
        let scales = inline.scales();
        self.scale = scales.0;
        let selected = self.inline_selection();
        let visible = self.inline_visible_rect();
        let toolbar = toolbar_frame(selected, bounds);
        let bands = selection::dim_bands(bounds, Some(selected)).unwrap_or_default();
        let mut root = div()
            .relative()
            .w(px(bounds.width))
            .h(px(bounds.height))
            .text_color(p.primary)
            .font_family(theme::ui_font())
            .track_focus(&self.focus)
            .cursor(gpui::CursorStyle::Arrow)
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                if !main_area::current(this.inline.as_ref().unwrap().id, window, cx) {
                    main_area::cancel(this.inline.as_ref().unwrap().id, cx);
                    cx.stop_propagation();
                }
            }))
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if !main_area::current(this.inline.as_ref().unwrap().id, window, cx) {
                    main_area::cancel(this.inline.as_ref().unwrap().id, cx);
                    cx.stop_propagation();
                } else if this.color_target.is_some()
                    && !this.color_input.read(cx).has_marked_text()
                    && matches!(event.keystroke.key.as_str(), "escape" | "enter")
                {
                    this.color_target = None;
                    window_focus_after_color(this, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.update_selection_adjustment(event.position, false, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.update_selection_adjustment(event.position, true, cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.update_selection_adjustment(event.position, true, cx);
                }),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        for band in bands {
                            window.paint_quad(gpui::fill(
                                Bounds::new(
                                    point(
                                        bounds.origin.x + px(band.x),
                                        bounds.origin.y + px(band.y),
                                    ),
                                    size(px(band.width), px(band.height)),
                                ),
                                gpui::black().opacity(0.34),
                            ));
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(selected.x))
                    .top(px(selected.y))
                    .w(px(selected.width))
                    .h(px(selected.height))
                    .overflow_hidden()
                    .child(self.annotation_surface(
                        visible,
                        scales,
                        (selected.width, selected.height),
                        p,
                        cx,
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left(px(selected.x))
                    .top(px(selected.y))
                    .w(px(selected.width))
                    .h(px(selected.height))
                    .border_2()
                    .border_color(p.accent),
            )
            .child(
                div()
                    .id("inline-area-toolbar")
                    .absolute()
                    .left(px(toolbar.x))
                    .top(px(toolbar.y))
                    .w(px(toolbar.width))
                    .h(px(toolbar.height))
                    .px(px(8.))
                    .py(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .bg(p.surface)
                    .rounded(px(10.))
                    .shadow_md()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .id("inline-area-toolbar-scroll")
                            .flex_1()
                            .min_w_0()
                            .overflow_x_scroll()
                            .child(self.toolbar(false, p, cx).min_w(px(650.))),
                    )
                    .child(
                        icon_button("inline-copy", "doc.on.doc", false, p).on_click(
                            cx.listener(|this, _, window, cx| this.copy(false, window, cx)),
                        ),
                    )
                    .child(
                        icon_button("inline-save", "square.and.arrow.down", false, p)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    )
                    .child(
                        icon_button("inline-close", "xmark", false, p)
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    )
                    .child(
                        icon_button("inline-finish", "checkmark", false, p)
                            .bg(p.accent_fill)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.copy(true, window, cx)),
                            ),
                    ),
            );
        if !self.status.is_empty() {
            root = root.child(
                div()
                    .absolute()
                    .left(px(toolbar.x))
                    .top(px((toolbar.bottom() + 8.).min(bounds.bottom() - 40.)))
                    .w(px(toolbar.width))
                    .p(px(6.))
                    .rounded(px(5.))
                    .bg(p.surface)
                    .text_size(px(11.))
                    .text_color(if self.error { p.danger } else { p.secondary })
                    .child(self.status.clone()),
            );
        }
        // Source order is deliberate: handles must win hit testing even where
        // a tall selection forces its toolbar inside the selection rectangle.
        for (index, handle) in SelectionHandle::ALL.into_iter().enumerate() {
            let at = handle.position(selected).expect("valid crop");
            let active = self
                .inline
                .as_ref()
                .and_then(|inline| inline.adjustment.as_ref())
                .is_some_and(|drag| drag.handle == Some(handle));
            root = root.child(
                div()
                    .id(("inline-area-handle", index))
                    .absolute()
                    .left(px(at.x - 11.))
                    .top(px(at.y - 11.))
                    .size(px(22.))
                    .occlude()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor(if handle.is_corner() {
                        gpui::CursorStyle::Crosshair
                    } else if handle.moves_left_edge() || handle.moves_right_edge() {
                        gpui::CursorStyle::ResizeLeftRight
                    } else {
                        gpui::CursorStyle::ResizeUpDown
                    })
                    .child(
                        div()
                            .size(px(if active { 14. } else { 11. }))
                            .rounded_full()
                            .border_1()
                            .border_color(p.accent)
                            .bg(gpui::white()),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            this.begin_selection_adjustment(Some(handle), event.position, cx);
                            window.focus(&this.focus);
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
        if self.open_menu.is_some() {
            root = root.child(self.menu(p, false, cx));
        }
        if self.color_target.is_some() {
            root = root.child(self.color_panel(p, cx));
        }
        if self.export_busy {
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(p.bg.opacity(0.65))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("Exporting the current screenshot…")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
            );
        }
        root
    }
}
