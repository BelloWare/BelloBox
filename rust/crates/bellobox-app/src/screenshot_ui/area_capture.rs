//! DEBUG-only frozen Area selector. All displayed pixels are generated in memory
//! before this window is created. This file has no native capture/overlay adapter,
//! clipboard, image-file, provider or OS input-injection path.
use super::{button, theme};
use crate::session::{JobToken, SessionJobs};
use bellobox_core::screenshot::{
    Point, PreviewTile, Rect, ScreenshotDocument,
    area::{AreaDisplayGeometry, AreaPhase, FrozenAreaSession},
    scroll::synthetic_page_frame,
    selection,
};
use gpui::{
    App, Bounds, Context, CursorStyle, FocusHandle, Image, ImageFormat, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Subscription, TitlebarOptions, Window,
    WindowBounds, WindowOptions, canvas, div, fill, img, point, prelude::*, px, size,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const WIDTH: f32 = 800.;
const HEIGHT: f32 = 500.;
fn display() -> AreaDisplayGeometry {
    AreaDisplayGeometry {
        display_id: 1,
        cocoa_frame: Rect::new(0., 0., WIDTH, HEIGHT),
        pixel_size: (1600, 1000),
        rotation_degrees: 0,
    }
}
#[derive(Default)]
struct FixtureLaunch {
    jobs: SessionJobs,
    pending: bool,
}
impl gpui::Global for FixtureLaunch {}
impl FixtureLaunch {
    fn cancel(&mut self) {
        self.jobs.cancel();
        self.pending = false;
    }
    fn accept_preparation(&mut self, token: JobToken) -> bool {
        if !self.jobs.accepts(token) {
            return false;
        }
        self.pending = false;
        true
    }
}
/// Any newer tool navigation invalidates a still-preparing selector.
pub(super) fn cancel_pending_launch(cx: &mut App) {
    if cx.try_global::<FixtureLaunch>().is_some() {
        cx.global_mut::<FixtureLaunch>().cancel();
    }
}
struct PreparedFixture {
    area: FrozenAreaSession,
    tiles: Vec<PreviewTile>,
}

/// A newer open cancels the preceding freeze generation. No selector window is
/// created before immutable pixels and tiles are ready.
pub(super) fn open_fixture(cx: &mut App) {
    let (token, cancellation) = {
        let launch = cx.default_global::<FixtureLaunch>();
        launch.pending = true;
        let token = launch.jobs.begin();
        (token, launch.jobs.cancellation())
    };
    let task = cx
        .background_executor()
        .spawn(async move { prepare_fixture(cancellation) });
    cx.spawn(async move |cx| {
        let prepared = task.await;
        let _ = cx.update(|cx| {
            let launch = cx.default_global::<FixtureLaunch>();
            if !launch.accept_preparation(token) {
                return;
            }
            #[cfg(target_os = "macos")]
            if !bello_platform::macos_native::application_is_active().unwrap_or(false) {
                return;
            }
            match prepared {
                Ok(prepared) => open_prepared(prepared, cx),
                Err(error) => open_error(error, cx),
            }
        });
    })
    .detach();
}
fn prepare_fixture(cancellation: Arc<AtomicBool>) -> Result<PreparedFixture, String> {
    let active = || {
        if cancellation.load(Ordering::Acquire) {
            Err("Area fixture preparation was cancelled.".to_owned())
        } else {
            Ok(())
        }
    };
    active()?;
    let geometry = display();
    let mut area = FrozenAreaSession::new(geometry).map_err(|e| e.to_string())?;
    let token = area
        .freeze_token()
        .ok_or("Area fixture is not ready to freeze.")?;
    let frame = synthetic_page_frame(0, geometry.pixel_size.0, geometry.pixel_size.1, 2200)
        .map_err(|e| e.to_string())?;
    active()?;
    let document = ScreenshotDocument::from_png(&frame.png().map_err(|e| e.to_string())?)?;
    active()?;
    let tiles = document.render_preview_tiles()?;
    active()?;
    if !area
        .accept_frozen(token, document, geometry)
        .map_err(|e| e.to_string())?
    {
        return Err("The synthetic freeze was superseded.".into());
    }
    Ok(PreparedFixture { area, tiles })
}
fn open_prepared(prepared: PreparedFixture, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(840.), px(680.)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(840.), px(680.))),
            is_resizable: false,
            titlebar: Some(TitlebarOptions {
                title: Some("Area · Frozen Synthetic Fixture — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| FrozenSelector::new(prepared, window, cx)),
    );
}
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    image: Arc<Image>,
}
struct FrozenSelector {
    area: FrozenAreaSession,
    tiles: Vec<Tile>,
    viewport: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    handoff_jobs: SessionJobs,
    preparing_handoff: bool,
    transferring: bool,
    has_been_active: bool,
    locked_rect: Option<Rect>,
    status: String,
    _activation: Subscription,
}
impl FrozenSelector {
    fn new(prepared: PreparedFixture, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = weak.update(cx, |this: &mut FrozenSelector, _| this.cancel_work());
            true
        });
        let activation = cx.observe_window_activation(window, |this: &mut Self, window, _| {
            if window.is_window_active() {
                this.has_been_active = true;
            } else if this.has_been_active && !this.transferring {
                // A preparation handoff has not opened an editor yet, so navigation
                // cancels that pending handoff too, rather than activating it later.
                this.area.focus_lost();
                this.cancel_work();
                window.remove_window();
            }
        });
        Self {
            area: prepared.area,
            tiles: prepared
                .tiles
                .into_iter()
                .map(|tile| Tile {
                    x: tile.x,
                    y: tile.y,
                    width: tile.width,
                    height: tile.height,
                    image: Arc::new(Image::from_bytes(ImageFormat::Png, tile.png)),
                })
                .collect(),
            viewport: Rc::new(Cell::new(Bounds::default())),
            focus,
            handoff_jobs: SessionJobs::default(),
            preparing_handoff: false,
            transferring: false,
            has_been_active: window.is_window_active(),
            locked_rect: None,
            status: String::new(),
            _activation: activation,
        }
    }
    fn cancel_work(&mut self) {
        self.handoff_jobs.cancel();
        self.area.cancel();
        self.preparing_handoff = false;
    }
    fn close(&mut self, window: &mut Window) {
        self.cancel_work();
        window.remove_window();
    }
    fn local(&self, position: gpui::Point<Pixels>) -> Option<Point> {
        let bounds = self.viewport.get();
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        if width <= 0. || height <= 0. {
            return None;
        }
        Some(Point::new(
            f32::from(position.x - bounds.origin.x) * WIDTH / width,
            f32::from(position.y - bounds.origin.y) * HEIGHT / height,
        ))
    }
    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.preparing_handoff {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        window.focus(&self.focus);
        self.has_been_active = true;
        if let Err(error) = self.area.begin_drag(point, display()) {
            self.status = error.to_string();
        } else {
            self.status.clear();
        }
        cx.notify();
    }
    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.preparing_handoff || self.area.phase() != AreaPhase::Dragging {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        if let Err(error) = self.area.update_drag(point, display()) {
            self.status = error.to_string();
        }
        cx.notify();
    }
    fn mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.preparing_handoff || self.area.phase() != AreaPhase::Dragging {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        let selection = match self.area.end_drag(point, display()) {
            Ok(Some(selection)) => selection,
            Ok(None) => {
                self.status = "Drag an area at least 8 × 8 points.".into();
                cx.notify();
                return;
            }
            Err(error) => {
                self.status = error.to_string();
                cx.notify();
                return;
            }
        };
        self.locked_rect = Some(selection.selection_local_points);
        self.preparing_handoff = true;
        let size = selection.editor.document().visible_dimensions();
        self.status = format!("Preparing {} × {} px in the editor…", size.0, size.1);
        let token = self.handoff_jobs.begin();
        let cancellation = self.handoff_jobs.cancellation();
        let task = cx.background_executor().spawn(async move {
            if cancellation.load(Ordering::Acquire) {
                return None;
            }
            // Only the existing optional local font preparation performs file I/O.
            // No synthetic pixels are regenerated, imported, captured or reencoded.
            let session = super::prepare_session(selection.editor);
            (!cancellation.load(Ordering::Acquire)).then_some(session)
        });
        cx.spawn_in(window, async move |this, cx| {
            let session = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.handoff_jobs.accepts(token) || !this.preparing_handoff {
                    return;
                }
                if !requester_is_active(window) {
                    this.close(window);
                    return;
                }
                let Some(session) = session else {
                    this.close(window);
                    return;
                };
                this.transferring = true;
                this.preparing_handoff = false;
                super::open_session(session, "Frozen Area · synthetic fixture", cx);
                window.remove_window();
            });
        })
        .detach();
        cx.notify();
    }
}
fn requester_is_active(window: &Window) -> bool {
    #[cfg(target_os = "macos")]
    {
        // A key window can survive app deactivation. Recheck the actual AppKit
        // application state immediately before handoff, without restoring focus.
        let application_active =
            bello_platform::macos_native::application_is_active().unwrap_or(false);
        super::native_capture_should_focus(application_active, window.is_window_active())
    }
    #[cfg(not(target_os = "macos"))]
    {
        window.is_window_active()
    }
}

fn selection_chrome(
    selected: Option<Rect>,
    locked: bool,
    (sx, sy): (f32, f32),
) -> (Option<Rect>, f32, Option<String>) {
    // CaptureOverlayController.refreshChrome removes empty selections first,
    // hides the badge when locked, and increases the locked border to 2.5 pt.
    let selected = selected.filter(|rect| rect.width > 0. && rect.height > 0.);
    let label = selected.filter(|_| !locked).map(|rect| {
        format!(
            "{} × {} px",
            (rect.width * sx) as u32,
            (rect.height * sy) as u32
        )
    });
    (selected, if locked { 2.5 } else { 2. }, label)
}

impl Render for FrozenSelector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        let (sx, sy) = display().pixels_per_point().expect("fixed valid fixture");
        let (selected, border_width, label) = selection_chrome(
            self.locked_rect.or_else(|| self.area.preview_rect()),
            self.locked_rect.is_some(),
            (sx, sy),
        );
        let bands = selection::dim_bands(display().local_bounds(), selected).unwrap_or_default();
        let bounds_cell = self.viewport.clone();
        let viewport = div()
            .id("area-frozen-viewport")
            .relative()
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .flex_none()
            .overflow_hidden()
            .cursor(CursorStyle::Crosshair)
            .children(self.tiles.iter().map(|tile| {
                img(tile.image.clone())
                    .absolute()
                    .left(px(tile.x as f32 / sx))
                    .top(px(tile.y as f32 / sy))
                    .w(px(tile.width as f32 / sx))
                    .h(px(tile.height as f32 / sy))
            }))
            .child(
                canvas(
                    move |bounds, _, _| bounds_cell.set(bounds),
                    move |bounds, _, window, _| {
                        let scale_x = f32::from(bounds.size.width) / WIDTH;
                        let scale_y = f32::from(bounds.size.height) / HEIGHT;
                        let at = |rect: Rect| {
                            Bounds::new(
                                point(
                                    bounds.origin.x + px(rect.x * scale_x),
                                    bounds.origin.y + px(rect.y * scale_y),
                                ),
                                size(px(rect.width * scale_x), px(rect.height * scale_y)),
                            )
                        };
                        for band in bands {
                            if band.width > 0. && band.height > 0. {
                                window.paint_quad(fill(at(band), gpui::black().opacity(0.34)));
                            }
                        }
                        if let Some(rect) = selected {
                            let edge = border_width;
                            for side in [
                                Rect::new(rect.x, rect.y, rect.width, edge.min(rect.height)),
                                Rect::new(
                                    rect.x,
                                    rect.bottom() - edge.min(rect.height),
                                    rect.width,
                                    edge.min(rect.height),
                                ),
                                Rect::new(rect.x, rect.y, edge.min(rect.width), rect.height),
                                Rect::new(
                                    rect.right() - edge.min(rect.width),
                                    rect.y,
                                    edge.min(rect.width),
                                    rect.height,
                                ),
                            ] {
                                window.paint_quad(fill(at(side), p.accent));
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .when_some(label, |d, label| {
                let rect = selected.expect("label has selection");
                d.child(
                    div()
                        .absolute()
                        .left(px(rect.x.min(WIDTH - 150.)))
                        .top(px((rect.y - 26.).max(0.)))
                        .h(px(22.))
                        .px(px(6.))
                        .rounded(px(4.))
                        .bg(gpui::black().opacity(0.65))
                        .text_color(gpui::white())
                        .text_size(px(11.))
                        .child(label),
                )
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down));
        div()
            .size_full()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(theme::ui_font())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, _| {
                if event.keystroke.key == "escape" {
                    this.close(window);
                }
            }))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Area capture · frozen synthetic fixture"),
            )
            .child(div().text_size(px(11.)).text_color(p.secondary).child(
                "App-owned pixels only · 800 × 500 points · 1600 × 1000 pixels · Escape cancels",
            ))
            .child(viewport)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .child(if self.status.is_empty() {
                                "Drag on the frozen page to open that area in the editor."
                                    .to_owned()
                            } else {
                                self.status.clone()
                            }),
                    )
                    .child(
                        button("area-fixture-cancel", "Cancel", p)
                            .on_click(cx.listener(|this, _, window, _| this.close(window))),
                    ),
            )
    }
}
struct PreparationError {
    text: String,
    focus: FocusHandle,
}
impl Render for PreparationError {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        div()
            .size_full()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(p.bg)
            .text_color(p.primary)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, _| {
                if event.keystroke.key == "escape" {
                    window.remove_window();
                }
            }))
            .child("Could not prepare the synthetic Area fixture.")
            .child(self.text.clone())
            .child(
                button("area-fixture-error-close", "Close", p)
                    .on_click(|_, window, _| window.remove_window()),
            )
    }
}
fn open_error(error: String, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(520.), px(180.)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        move |window, cx| {
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                PreparationError { text: error, focus }
            })
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_cancels_pending_launch_and_newer_generation_wins() {
        let mut launch = FixtureLaunch {
            pending: true,
            ..Default::default()
        };
        let old = launch.jobs.begin();
        let flag = launch.jobs.cancellation();
        launch.cancel();
        assert!(!launch.pending);
        assert!(flag.load(Ordering::Acquire));
        assert!(!launch.jobs.accepts(old));
        let newer = launch.jobs.begin();
        launch.pending = true;
        assert!(!launch.accept_preparation(old));
        assert!(launch.pending);
        assert!(launch.accept_preparation(newer));
        assert!(!launch.pending);
        assert!(!launch.jobs.accepts(old));
    }
    #[test]
    fn macos_handoff_requires_active_application_and_requester_key_window() {
        for (application_active, requester_key, expected) in [
            (false, false, false),
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            assert_eq!(
                super::super::native_capture_should_focus(application_active, requester_key),
                expected
            );
        }
    }
    #[test]
    fn chrome_omits_empty_and_locked_badges_and_matches_source_border_width() {
        for rect in [Rect::new(1., 2., 0., 100.), Rect::new(1., 2., 100., 0.)] {
            let (selected, _, label) = selection_chrome(Some(rect), false, (2., 3.));
            assert!(selected.is_none());
            assert!(label.is_none());
        }
        let rect = Rect::new(1., 2., 10.4, 20.2);
        let (selected, border, label) = selection_chrome(Some(rect), false, (2., 3.));
        assert_eq!(selected, Some(rect));
        assert_eq!(border, 2.);
        assert_eq!(label.as_deref(), Some("20 × 60 px"));
        let (selected, border, label) = selection_chrome(Some(rect), true, (2., 3.));
        assert_eq!(selected, Some(rect));
        assert_eq!(border, 2.5);
        assert!(label.is_none());
    }
    #[test]
    fn prepared_fixture_is_tiled_frozen_and_example_crop_is_clean() {
        let mut fixture = prepare_fixture(Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(fixture.area.phase(), AreaPhase::Selecting);
        assert!(fixture.tiles.len() >= 2);
        assert!(
            fixture
                .tiles
                .iter()
                .all(|t| t.width <= 1024 && t.height <= 1024)
        );
        fixture
            .area
            .begin_drag(Point::new(100., 80.), display())
            .unwrap();
        let selected = fixture
            .area
            .end_drag(Point::new(400., 280.), display())
            .unwrap()
            .unwrap();
        assert_eq!(selected.editor.document().visible_dimensions(), (600, 400));
        assert!(!selected.editor.has_edits());
        assert!(!selected.editor.can_undo());
    }
    #[test]
    fn cancelled_fixture_preparation_does_not_make_a_selector() {
        assert!(prepare_fixture(Arc::new(AtomicBool::new(true))).is_err());
    }
}
