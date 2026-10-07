//! DEBUG-only frozen Area/Window selector. All displayed pixels are generated in memory
//! before this window is created. This file has no native capture/overlay adapter,
//! clipboard, image-file, provider or OS input-injection path.
use super::{button, theme};
use crate::session::{JobToken, SessionJobs};
use bellobox_core::screenshot::{
    Point, PreviewTile, Rect, ScreenshotDocument,
    area::{AreaDisplayGeometry, AreaPhase, AreaSelection, FrozenAreaSession},
    scroll::synthetic_page_frame,
    selection,
    window::{
        FrozenWindowCommit, FrozenWindowCommitToken, FrozenWindowPhase, FrozenWindowSession,
        synthetic_window_fixture,
    },
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
    visible: Option<gpui::WindowHandle<FrozenSelector>>,
}
impl gpui::Global for FixtureLaunch {}
impl FixtureLaunch {
    fn cancel(&mut self) {
        self.jobs.cancel();
        self.pending = false;
    }
    fn accept_preparation(&mut self, token: JobToken) -> bool {
        if !self.pending || !self.jobs.accepts(token) {
            return false;
        }
        self.pending = false;
        true
    }
}
/// Newer navigation invalidates preparation and retires the prior visible fixture.
pub(super) fn cancel_pending_launch(cx: &mut App) {
    if cx.try_global::<FixtureLaunch>().is_some() {
        let launch = cx.global_mut::<FixtureLaunch>();
        launch.cancel();
        if let Some(previous) = launch.visible.take() {
            // Defer logical retirement to avoid reentering a currently borrowed
            // selector. Its global cancellation flag already rejects publication.
            cx.defer(move |cx| {
                let _ = previous.update(cx, |view, window, _| view.close(window));
            });
        }
    }
}
struct PreparedFixture {
    selection: FixtureSelection,
    tiles: Vec<PreviewTile>,
}
enum FixtureSelection {
    Area(FrozenAreaSession),
    Window(FrozenWindowSession),
}
impl FixtureSelection {
    fn is_window(&self) -> bool {
        matches!(self, Self::Window(_))
    }
    fn cancel(&mut self) {
        match self {
            Self::Area(s) => s.cancel(),
            Self::Window(s) => s.cancel(),
        }
    }
    fn focus_lost(&mut self) {
        match self {
            Self::Area(s) => {
                s.focus_lost();
            }
            Self::Window(s) => s.focus_lost(),
        }
    }
    fn preview_rect(&self) -> Option<Rect> {
        match self {
            Self::Area(s) => s.preview_rect(),
            Self::Window(s) => s.preview_rect(),
        }
    }
    fn pressing(&self) -> bool {
        match self {
            Self::Area(s) => s.phase() == AreaPhase::Dragging,
            Self::Window(s) => s.phase() == FrozenWindowPhase::Pressed,
        }
    }
    fn begin(&mut self, point: Point) -> Result<(), String> {
        match self {
            Self::Area(s) => s.begin_drag(point, display()).map_err(|e| e.to_string()),
            Self::Window(s) => s.begin_press(point, display()).map_err(|e| e.to_string()),
        }
    }
    fn movement(&mut self, point: Point) -> Result<(), String> {
        match self {
            Self::Area(s) if s.phase() == AreaPhase::Dragging => {
                s.update_drag(point, display()).map_err(|e| e.to_string())
            }
            Self::Area(_) => Ok(()),
            Self::Window(s) => s.hover(point, display()).map_err(|e| e.to_string()),
        }
    }
    fn end(&mut self, point: Point) -> Result<Option<FixtureCommit>, String> {
        match self {
            Self::Area(s) => s
                .end_drag(point, display())
                .map(|v| v.map(FixtureCommit::Area))
                .map_err(|e| e.to_string()),
            Self::Window(s) => s
                .end_press(point, display())
                .map(|v| v.map(FixtureCommit::Window))
                .map_err(|e| e.to_string()),
        }
    }
    fn accepts(&mut self, token: Option<FrozenWindowCommitToken>) -> bool {
        match (self, token) {
            (Self::Area(s), None) => {
                s.phase() == AreaPhase::Committed && s.validate_topology(display()).is_ok()
            }
            (Self::Window(s), Some(token)) => s.accepts_commit(token, display()).unwrap_or(false),
            _ => false,
        }
    }
}
enum FixtureCommit {
    Area(AreaSelection),
    Window(FrozenWindowCommit),
}
impl FixtureCommit {
    fn token(&self) -> Option<FrozenWindowCommitToken> {
        match self {
            Self::Area(_) => None,
            Self::Window(s) => Some(s.token()),
        }
    }
    fn rect(&self) -> Rect {
        match self {
            Self::Area(s) => s.selection_local_points,
            Self::Window(s) => s.candidate().frame_local_points,
        }
    }
    fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Area(s) => s.editor.document().visible_dimensions(),
            Self::Window(s) => (s.pixel_crop().width as u32, s.pixel_crop().height as u32),
        }
    }
    fn materialize(
        self,
        cancellation: &AtomicBool,
    ) -> Result<bellobox_core::screenshot::ScreenshotEditSession, String> {
        match self {
            Self::Area(s) => Ok(s.editor),
            Self::Window(s) => s
                .materialize(cancellation)
                .map(|s| s.editor)
                .map_err(|e| e.to_string()),
        }
    }
}

/// A newer open cancels the preceding freeze generation. No selector window is
/// created before immutable pixels and tiles are ready.
pub(super) fn open_fixture(cx: &mut App) {
    open_fixture_mode(false, cx);
}
pub(super) fn open_window_fixture(cx: &mut App) {
    open_fixture_mode(true, cx);
}
fn open_fixture_mode(window_mode: bool, cx: &mut App) {
    cancel_pending_launch(cx);
    let (token, cancellation) = {
        let launch = cx.default_global::<FixtureLaunch>();
        launch.pending = true;
        let token = launch.jobs.begin();
        (token, launch.jobs.cancellation())
    };
    let worker_cancellation = cancellation.clone();
    let task = cx.background_executor().spawn(async move {
        if window_mode {
            prepare_window_fixture(worker_cancellation)
        } else {
            prepare_fixture(worker_cancellation)
        }
    });
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
                Ok(prepared) => open_prepared(prepared, token, cancellation, cx),
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
    Ok(PreparedFixture {
        selection: FixtureSelection::Area(area),
        tiles,
    })
}
fn prepare_window_fixture(cancellation: Arc<AtomicBool>) -> Result<PreparedFixture, String> {
    if cancellation.load(Ordering::Acquire) {
        return Err("Window fixture preparation was cancelled.".into());
    }
    let (geometry, catalog, document) = synthetic_window_fixture().map_err(|e| e.to_string())?;
    let mut selection =
        FrozenWindowSession::new(geometry, &catalog, 999).map_err(|e| e.to_string())?;
    let token = selection
        .freeze_token()
        .ok_or("Window fixture is not ready to freeze.")?;
    let tiles = document.render_preview_tiles()?;
    if cancellation.load(Ordering::Acquire) {
        return Err("Window fixture preparation was cancelled.".into());
    }
    if !selection
        .accept_frozen(token, document, geometry)
        .map_err(|e| e.to_string())?
    {
        return Err("Window fixture freeze was superseded.".into());
    }
    Ok(PreparedFixture {
        selection: FixtureSelection::Window(selection),
        tiles,
    })
}
fn open_prepared(
    prepared: PreparedFixture,
    token: JobToken,
    cancellation: Arc<AtomicBool>,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(840.), px(680.)), cx);
    let window_mode = prepared.selection.is_window();
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(840.), px(680.))),
            is_resizable: false,
            titlebar: Some(TitlebarOptions {
                title: Some(
                    if window_mode {
                        "Window · Frozen Synthetic Fixture — Bello Box"
                    } else {
                        "Area · Frozen Synthetic Fixture — Bello Box"
                    }
                    .into(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            cx.new(|cx| FrozenSelector::new(prepared, token, cancellation, window, cx))
        },
    );
    match opened {
        Ok(handle) => cx.default_global::<FixtureLaunch>().visible = Some(handle),
        Err(error) => open_error(error.to_string(), cx),
    }
}
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    image: Arc<Image>,
}
struct FrozenSelector {
    selection: FixtureSelection,
    launch_token: JobToken,
    launch_cancellation: Arc<AtomicBool>,
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
    fn new(
        prepared: PreparedFixture,
        launch_token: JobToken,
        launch_cancellation: Arc<AtomicBool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
                this.selection.focus_lost();
                this.cancel_work();
                window.remove_window();
            }
        });
        Self {
            selection: prepared.selection,
            launch_token,
            launch_cancellation,
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
        self.selection.cancel();
        self.preparing_handoff = false;
    }
    fn close(&mut self, window: &mut Window) {
        self.cancel_work();
        window.remove_window();
    }
    fn launch_is_current(&self, cx: &App) -> bool {
        !self.launch_cancellation.load(Ordering::Acquire)
            && cx
                .try_global::<FixtureLaunch>()
                .is_some_and(|launch| launch.jobs.accepts(self.launch_token))
    }
    fn guard_launch(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.launch_is_current(cx) {
            true
        } else {
            self.close(window);
            false
        }
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
        if self.preparing_handoff || !self.guard_launch(window, cx) {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        window.focus(&self.focus);
        self.has_been_active = true;
        if let Err(error) = self.selection.begin(point) {
            self.status = error.to_string();
        } else {
            self.status.clear();
        }
        cx.notify();
    }
    fn mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.preparing_handoff || !self.guard_launch(window, cx) {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        if let Err(error) = self.selection.movement(point) {
            self.status = error.to_string();
        }
        cx.notify();
    }
    fn mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.preparing_handoff || !self.guard_launch(window, cx) || !self.selection.pressing() {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        let selection = match self.selection.end(point) {
            Ok(Some(selection)) => selection,
            Ok(None) => {
                self.status = if self.selection.is_window() {
                    "Click a highlighted window; dragging does not select an area."
                } else {
                    "Drag an area at least 8 × 8 points."
                }
                .into();
                cx.notify();
                return;
            }
            Err(error) => {
                self.status = error.to_string();
                cx.notify();
                return;
            }
        };
        self.locked_rect = Some(selection.rect());
        let refresh = match &selection {
            FixtureCommit::Window(commit)
                if std::env::var_os("BELLOBOX_WINDOW_REFRESH_FIXTURE").is_some() =>
            {
                match super::window_refresh::Request::fixture(
                    commit,
                    self.launch_cancellation.clone(),
                ) {
                    Ok(request) => Some(request),
                    Err(error) => {
                        self.status = error;
                        cx.notify();
                        return;
                    }
                }
            }
            _ => None,
        };
        let selection_token = selection.token();
        self.preparing_handoff = true;
        let size = selection.dimensions();
        self.status = format!("Preparing {} × {} px in the editor…", size.0, size.1);
        let token = self.handoff_jobs.begin();
        let cancellation = self.handoff_jobs.cancellation();
        let task = cx.background_executor().spawn(async move {
            if cancellation.load(Ordering::Acquire) {
                return Ok(None);
            }
            // A Window copies only its bound frozen rectangle on this worker.
            // Area keeps its existing full document and clean crop. Neither captures.
            let session = selection.materialize(&cancellation)?;
            let session = super::prepare_session(session);
            Ok::<_, String>((!cancellation.load(Ordering::Acquire)).then_some(session))
        });
        cx.spawn_in(window, async move |this, cx| {
            let session = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.handoff_jobs.accepts(token) || !this.preparing_handoff {
                    return;
                }
                if !this.launch_is_current(cx)
                    || !requester_is_active(window)
                    || !this.selection.accepts(selection_token)
                {
                    this.close(window);
                    return;
                }
                let session = match session {
                    Ok(Some(session)) => session,
                    Ok(None) => {
                        this.close(window);
                        return;
                    }
                    Err(error) => {
                        this.close(window);
                        open_error(error, cx);
                        return;
                    }
                };
                this.transferring = true;
                this.preparing_handoff = false;
                super::open_presented_session(
                    session,
                    if this.selection.is_window() {
                        "Frozen Window · synthetic visible pixels"
                    } else {
                        "Frozen Area · synthetic fixture"
                    },
                    super::CapturePresentation {
                        window_refresh: refresh,
                        ..Default::default()
                    },
                    cx,
                );
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
            self.locked_rect.or_else(|| self.selection.preview_rect()),
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
                    .child(if self.selection.is_window() {
                        "Window selection · frozen synthetic fixture"
                    } else {
                        "Area capture · frozen synthetic fixture"
                    }),
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
                                if self.selection.is_window() {
                                    "Click a window. The frozen crop keeps any visible overlap."
                                } else {
                                    "Drag on the frozen page to open that area in the editor."
                                }
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
            .child("Could not prepare the synthetic capture fixture.")
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
        let FixtureSelection::Area(area) = &mut fixture.selection else {
            panic!("expected Area fixture");
        };
        assert_eq!(area.phase(), AreaPhase::Selecting);
        assert!(fixture.tiles.len() >= 2);
        assert!(
            fixture
                .tiles
                .iter()
                .all(|t| t.width <= 1024 && t.height <= 1024)
        );
        area.begin_drag(Point::new(100., 80.), display()).unwrap();
        let selected = area
            .end_drag(Point::new(400., 280.), display())
            .unwrap()
            .unwrap();
        assert_eq!(selected.editor.document().visible_dimensions(), (600, 400));
        assert!(!selected.editor.has_edits());
        assert!(!selected.editor.can_undo());
    }
    #[test]
    fn accepted_visible_launch_is_still_invalidated_by_a_newer_open() {
        let mut launch = FixtureLaunch::default();
        let old = launch.jobs.begin();
        let old_flag = launch.jobs.cancellation();
        launch.pending = true;
        assert!(launch.accept_preparation(old));
        assert!(!launch.accept_preparation(old));
        assert!(launch.jobs.accepts(old));
        let newer = launch.jobs.begin();
        launch.pending = true;
        assert!(old_flag.load(Ordering::Acquire));
        assert!(!launch.jobs.accepts(old));
        assert!(launch.accept_preparation(newer));
    }
    #[test]
    fn prepared_window_fixture_uses_the_same_tiles_and_a_window_sized_clean_commit() {
        let mut fixture = prepare_window_fixture(Arc::new(AtomicBool::new(false))).unwrap();
        assert!(fixture.selection.is_window());
        assert!(fixture.tiles.len() >= 2);
        fixture.selection.movement(Point::new(150., 200.)).unwrap();
        assert_eq!(
            fixture.selection.preview_rect(),
            Some(Rect::new(100., 170., 420., 230.))
        );
        fixture.selection.begin(Point::new(150., 200.)).unwrap();
        let commit = fixture
            .selection
            .end(Point::new(150., 200.))
            .unwrap()
            .unwrap();
        let token = commit.token();
        let session = commit.materialize(&AtomicBool::new(false)).unwrap();
        assert_eq!(session.document().dimensions(), (840, 460));
        assert_eq!(session.document().crop_rect(), None);
        assert!(!session.can_undo());
        assert!(fixture.selection.accepts(token));
        fixture.selection.cancel();
        assert!(!fixture.selection.accepts(token));
        assert!(prepare_window_fixture(Arc::new(AtomicBool::new(true))).is_err());
    }
    #[test]
    fn cancelled_fixture_preparation_does_not_make_a_selector() {
        assert!(prepare_fixture(Arc::new(AtomicBool::new(true))).is_err());
    }
}
