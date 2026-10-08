//! Frozen full-display selector. Dragging changes only chrome; immutable tiled
//! pixels and the coordinator remain owned throughout the inline editing phase.
use super::*;

#[cfg(any(debug_assertions, test))]
pub(super) mod ai_fixture;

use bellobox_core::screenshot::window::{
    FrozenWindowCommit, FrozenWindowCommitToken, FrozenWindowPhase, FrozenWindowSession,
};
use bellobox_core::screenshot::{
    Point, ScreenshotEditSession,
    area::{AreaPhase, AreaSelection},
    selection,
};
use gpui::{
    Bounds, Context, CursorStyle, FocusHandle, Image, ImageFormat, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Subscription, WindowBounds, WindowKind,
    WindowOptions, canvas, div, fill, img, point, prelude::*, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The shared selector keeps one frozen image owner for either selection policy.
pub(super) enum Selection {
    Area(FrozenAreaSession),

    #[cfg(any(debug_assertions, test))]
    AiFixture(ai_fixture::Selection),

    Window(
        FrozenWindowSession,
        Arc<super::super::window_workflow::Source>,
    ),
}
impl Selection {
    fn preview_rect(&self) -> Option<Rect> {
        match self {
            Self::Area(area) => area.preview_rect(),
            #[cfg(any(debug_assertions, test))]
            Self::AiFixture(fixture) => fixture.preview_rect(),

            Self::Window(window, _) => window.preview_rect(),
        }
    }
    fn begin(&mut self, point: Point, display: AreaDisplayGeometry) -> Result<(), String> {
        match self {
            Self::Area(area) => area.begin_drag(point, display).map_err(|e| e.to_string()),
            #[cfg(any(debug_assertions, test))]
            Self::AiFixture(fixture) => fixture.begin(point, display),

            Self::Window(window, _) => window
                .begin_press(point, display)
                .map_err(|e| e.to_string()),
        }
    }
    fn movement(&mut self, point: Point, display: AreaDisplayGeometry) -> Result<(), String> {
        match self {
            Self::Area(area) if area.phase() == AreaPhase::Dragging => {
                area.update_drag(point, display).map_err(|e| e.to_string())
            }
            Self::Area(_) => Ok(()),
            #[cfg(any(debug_assertions, test))]
            Self::AiFixture(fixture) => fixture.movement(point, display),

            Self::Window(window, _) => window.hover(point, display).map_err(|e| e.to_string()),
        }
    }
    fn end(
        &mut self,
        point: Point,
        display: AreaDisplayGeometry,
    ) -> Result<Option<Commit>, String> {
        match self {
            Self::Area(area) if area.phase() == AreaPhase::Dragging => area
                .end_drag(point, display)
                .map(|c| c.map(Commit::Area))
                .map_err(|e| e.to_string()),
            Self::Area(_) => Ok(None),
            #[cfg(any(debug_assertions, test))]
            Self::AiFixture(fixture) => fixture.end(point, display),

            Self::Window(window, source) if window.phase() == FrozenWindowPhase::Pressed => window
                .end_press(point, display)
                .map(|c| c.map(|c| Commit::Window(c, source.clone())))
                .map_err(|e| e.to_string()),

            Self::Window(_, _) => Ok(None),
        }
    }
    fn accepts(&mut self, editor: &PreparedEditor, display: AreaDisplayGeometry) -> bool {
        match self {
            Self::Area(area) => {
                let _ = editor;
                area.phase() == AreaPhase::Committed && area.validate_topology(display).is_ok()
            }
            #[cfg(any(debug_assertions, test))]
            Self::AiFixture(fixture) => fixture.accepts(editor, display),

            Self::Window(window, _) => editor
                .token
                .is_some_and(|token| window.accepts_commit(token, display).unwrap_or(false)),
        }
    }
}
enum Commit {
    Area(AreaSelection),

    #[cfg(any(debug_assertions, test))]
    AiFixture(ai_fixture::Commit),

    Window(
        FrozenWindowCommit,
        Arc<super::super::window_workflow::Source>,
    ),
}
struct PreparedEditor {
    session: ScreenshotEditSession,
    fixed_frame: Option<Rect>,

    #[cfg(any(debug_assertions, test))]
    fixture: Option<ai_fixture::Authority>,

    token: Option<FrozenWindowCommitToken>,

    refresh: Option<super::super::window_refresh::Request>,
}
impl Commit {
    fn rect(&self) -> Rect {
        match self {
            Self::Area(area) => area.selection_local_points,
            #[cfg(any(debug_assertions, test))]
            Self::AiFixture(fixture) => fixture.rect(),

            Self::Window(window, _) => window.candidate().frame_local_points,
        }
    }
    fn prepare(self, cancellation: Arc<AtomicBool>) -> Result<PreparedEditor, String> {
        let _ = &cancellation;
        match self {
            Self::Area(area) => Ok(PreparedEditor {
                session: super::super::prepare_session(area.editor),
                fixed_frame: None,

                #[cfg(any(debug_assertions, test))]
                fixture: None,

                token: None,

                refresh: None,
            }),

            #[cfg(any(debug_assertions, test))]
            Self::AiFixture(fixture) => fixture.prepare(),

            Self::Window(commit, source) => {
                let fixed_frame = commit.candidate().frame_local_points;
                let token = commit.token();
                // Fidelity refresh is optional. Failure preserves the exact frozen crop.
                let refresh = source
                    .pending_request(&commit, cancellation.clone())
                    .ok()
                    .map(|(context, acquisition)| {
                        super::super::window_refresh::Request::pending(
                            context,
                            cancellation.clone(),
                            acquisition,
                        )
                    });
                let session = commit
                    .materialize(&cancellation)
                    .map_err(|e| e.to_string())?
                    .editor;
                Ok(PreparedEditor {
                    session: super::super::prepare_session(session),
                    fixed_frame: Some(fixed_frame),
                    #[cfg(any(debug_assertions, test))]
                    fixture: None,
                    token: Some(token),
                    refresh,
                })
            }
        }
    }
}

struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    image: Arc<Image>,
}
pub(super) struct MainAreaSelector {
    id: u64,
    layout: host::MainDisplayOverlayLayout,
    selection: Option<Selection>,
    tiles: Vec<Tile>,
    viewport: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    presented: bool,
    retired: bool,
    preparing_editor: bool,
    editor: Option<gpui::Entity<super::super::ScreenshotEditor>>,
    locked: Option<Rect>,
    quit_notice: bool,
    _activation: Subscription,
}
fn owns_key(window: &Window) -> bool {
    native::context(window).is_ok_and(|c| c.key_window == host::OwnedKeyWindow::Requester)
}
pub(super) fn open(
    id: u64,
    layout: host::MainDisplayOverlayLayout,
    prepared: PreparedArea,
    cx: &mut App,
) -> Result<(), String> {
    // Supplied image identities are not native/GPUI display identifiers.
    // Production still requires the exact validated primary display.
    let display = if is_supplied(id, cx) {
        cx.displays().into_iter().next()
    } else {
        cx.displays()
            .into_iter()
            .find(|d| u32::from(d.id()) == layout.display.id)
    };
    let Some(display) = display else {
        mark_cancelled(id, cx);
        discard(id, prepared, cx);
        return Err(host::OverlayError::DisplayChanged.to_string());
    };
    // Preserve worker-owned pixels for off-thread disposal if native open fails
    // before GPUI consumes the root-view constructor.
    let pending = Rc::new(RefCell::new(Some(prepared)));
    let input = pending.clone();
    let handle = cx
        .open_window(
            WindowOptions {
                kind: WindowKind::PopUp,
                show: is_supplied(id, cx),
                focus: is_supplied(id, cx),
                titlebar: None,
                tabbing_identifier: None,
                display_id: Some(display.id()),
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(
                        px(layout.cocoa_frame.size.width as f32),
                        px(layout.cocoa_frame.size.height as f32),
                    ),
                ))),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    MainAreaSelector::new(
                        id,
                        layout,
                        input.borrow_mut().take().expect("one prepared selector"),
                        window,
                        cx,
                    )
                })
            },
        )
        .map_err(|error| {
            mark_cancelled(id, cx);
            if let Some(prepared) = pending.borrow_mut().take() {
                discard(id, prepared, cx);
            }
            error.to_string()
        })?;
    if let Some(run) = active_mut(id, cx) {
        run.transaction.selector = Some(handle.into());
    }
    let result = handle
        .update(cx, |_, window, cx| {
            if is_supplied(id, cx) {
                return Ok(());
            }
            native::configure(
                window,
                layout,
                capture::CaptureSize {
                    width: layout.cocoa_frame.size.width,
                    height: layout.cocoa_frame.size.height,
                },
            )
        })
        .map_err(|e| e.to_string())
        .and_then(|r| r);
    if let Err(error) = result {
        let _ = handle.update(cx, |view, window, cx| view.retire(window, cx));
        return Err(error);
    }
    // Pinned GPUI's synchronous native resize callback may be rejected while
    // borrowed. Deferral is NOT proof of replay: validate cached viewport/scale
    // explicitly; mismatch closes this hidden popup, never a titled fallback.
    cx.defer(move |cx| {
        let result = handle
            .update(cx, |view, window, cx| view.present(window, cx))
            .map_err(|e| e.to_string())
            .and_then(|r| r);
        if let Err(error) = result {
            mark_cancelled(id, cx);
            report(id, error, cx);
            let _ = handle.update(cx, |view, window, cx| view.retire(window, cx));
            finish_if_ready(id, cx);
        }
    });
    Ok(())
}
impl MainAreaSelector {
    fn new(
        id: u64,
        layout: host::MainDisplayOverlayLayout,
        prepared: PreparedArea,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        crate::shutdown::guard_quit(window, cx, |this: &mut Self, window, cx| {
            if crate::shutdown::is_quit_feedback_window(window, cx) {
                this.quit_notice = true;
                cx.notify();
                crate::shutdown::QuitAdmission::Refused
            } else {
                crate::shutdown::QuitAdmission::Explain("Finish or cancel the capture overlay.")
            }
        });
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = weak.update(cx, |view: &mut Self, cx| view.retire(window, cx));
            crate::shutdown::allow_close(window, cx)
        });
        let activation = cx.observe_window_activation(window, |view: &mut Self, window, cx| {
            if view.presented
                && view.locked.is_none()
                && ((!is_supplied(view.id, cx) && !owns_key(window))
                    || !window.is_window_active()
                    || !current(view.id, window, cx))
            {
                view.retire(window, cx);
            }
        });
        cx.on_release(|view: &mut Self, cx| {
            mark_cancelled(view.id, cx);
            view.release_pixels(cx);
        })
        .detach();
        Self {
            id,
            layout,
            selection: Some(prepared.selection),
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
            presented: false,
            retired: false,
            preparing_editor: false,
            editor: None,
            locked: None,
            quit_notice: false,
            _activation: activation,
        }
    }
    fn present(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        if is_supplied(self.id, cx) {
            if !current_requester(self.id, cx) {
                return Err(host::OverlayError::NavigationChanged.to_string());
            }
            self.presented = true;
            #[cfg(any(debug_assertions, test))]
            if let Some(Selection::AiFixture(fixture)) = self.selection.as_mut()
                && let Some(commit) = fixture.fixed_window_commit()
            {
                self.prepare_editor(commit, window, cx);
            }
            cx.notify();
            return Ok(());
        }
        let size = window.viewport_size();
        let viewport = capture::CaptureSize {
            width: f32::from(size.width) as f64,
            height: f32::from(size.height) as f64,
        };
        validate_viewport(
            self.layout,
            viewport,
            f64::from(window.scale_factor()),
            window.display(cx).map(|d| u32::from(d.id())),
        )?;
        if !current_requester(self.id, cx) {
            return Err(host::OverlayError::NavigationChanged.to_string());
        }
        let (requester, cancellation) = active_mut(self.id, cx)
            .map(|run| {
                (
                    run.transaction.requester,
                    run.transaction.cancellation.clone(),
                )
            })
            .ok_or_else(|| host::OverlayError::NavigationChanged.to_string())?;
        // The outer popup logical update remains live while this distinct
        // requester update borrows its owner. Both raw borrows last only for the
        // synchronous helper call. Never queue GPUI activate_window here.
        requester
            .update(cx, |_, requester_window, _| {
                native::present(
                    window,
                    requester_window,
                    self.layout,
                    viewport,
                    &capture::CaptureCancellation::from_flag(cancellation),
                )
            })
            .map_err(|e| e.to_string())??;
        self.presented = true;
        cx.notify();
        Ok(())
    }
    fn release_pixels(&mut self, cx: &mut App) {
        if let Some(editor) = self.editor.take() {
            editor.update(cx, |editor, cx| editor.retire_inline(cx));
        }
        if let Some(run) = active_mut(self.id, cx) {
            run.transaction.selector = None;
        }
        if self.selection.is_some() || !self.tiles.is_empty() {
            discard(
                self.id,
                (self.selection.take(), std::mem::take(&mut self.tiles)),
                cx,
            );
        } else {
            finish_if_ready(self.id, cx);
        }
    }
    pub(super) fn retire(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quit_notice = false;
        if self.retired {
            return;
        }
        self.retired = true;
        mark_cancelled(self.id, cx);
        // GPUI remove_window queues native close. Source cancel orders screen-
        // level panels out immediately, before releasing their image/view state.
        if !is_supplied(self.id, cx)
            && let Err(error) = native::hide(window)
        {
            cleanup_notice(
                self.id,
                format!(
                    "Area selector could not be hidden safely: {error} Its close has been requested."
                ),
                cx,
            );
        }
        self.presented = false;
        self.release_pixels(cx);
        crate::shutdown::close_window(window, cx);
    }
    fn guard(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.presented
            && (self.locked.is_some() || is_supplied(self.id, cx) || owns_key(window))
            && current(self.id, window, cx)
        {
            true
        } else {
            self.retire(window, cx);
            false
        }
    }
    fn local(&self, position: gpui::Point<Pixels>) -> Option<Point> {
        let bounds = self.viewport.get();
        if f32::from(bounds.size.width) <= 0. || f32::from(bounds.size.height) <= 0. {
            return None;
        }
        Some(Point::new(
            f32::from(position.x - bounds.origin.x),
            f32::from(position.y - bounds.origin.y),
        ))
    }
    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.quit_notice) {
            cx.notify();
        }
        if self.locked.is_some() || !self.guard(window, cx) {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        window.focus(&self.focus);
        if self
            .selection
            .as_mut()
            .is_none_or(|selection| selection.begin(point, geometry(self.layout)).is_err())
        {
            self.retire(window, cx);
            return;
        }
        cx.notify();
    }
    fn mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked.is_some() || !self.guard(window, cx) {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        if let Some(selection) = self.selection.as_mut()
            && selection.movement(point, geometry(self.layout)).is_err()
        {
            self.retire(window, cx);
            return;
        }
        cx.notify();
    }
    fn mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.quit_notice) {
            cx.notify();
        }
        if self.locked.is_some() || !self.guard(window, cx) {
            return;
        }
        let Some(point) = self.local(event.position) else {
            return;
        };
        let Some(selection) = self.selection.as_mut() else {
            return;
        };
        let selection = match selection.end(point, geometry(self.layout)) {
            Ok(Some(selection)) => selection,
            Ok(None) => {
                cx.notify();
                return;
            }
            Err(_) => {
                self.retire(window, cx);
                return;
            }
        };
        self.prepare_editor(selection, window, cx);
    }
    fn prepare_editor(&mut self, selection: Commit, window: &mut Window, cx: &mut Context<Self>) {
        self.locked = Some(selection.rect());
        lock_selection(self.id, cx);
        self.preparing_editor = true;
        let id = self.id;
        let selector = window.window_handle();
        let Some(run) = active_mut(id, cx) else {
            self.retire(window, cx);
            return;
        };
        run.transaction.start_worker();
        let cancellation = run.transaction.cancellation.clone();
        let task = cx.background_executor().spawn(async move {
            super::super::caught_capture(|| {
                if cancellation.load(Ordering::Acquire) {
                    return Ok(None);
                }
                // Area retains its full image; Window physically copies only the
                // bound rectangle. Font/image preparation stays on this worker.
                let editor = selection.prepare(cancellation.clone())?;
                Ok((!cancellation.load(Ordering::Acquire)).then_some(editor))
            })
        });
        let app: &mut App = cx;
        app.spawn(async move |cx| {
            let result = task.await;
            let _ = cx.update(|cx| editor_prepared(id, selector, result, cx));
        })
        .detach();
        cx.notify();
    }
}
fn editor_prepared(
    id: u64,
    selector: AnyWindowHandle,
    result: Result<Option<PreparedEditor>, String>,
    cx: &mut App,
) {
    if let Some(run) = active_mut(id, cx) {
        run.transaction.finish_worker();
    }
    let mut result = Some(result);
    let updated = selector.downcast::<MainAreaSelector>().and_then(|handle| {
        handle
            .update(cx, |view, window, cx| {
                if !view.preparing_editor || !view.guard(window, cx) {
                    return;
                }
                match result.take().expect("one prepared inline editor") {
                    Ok(Some(prepared)) => {
                        if !view.selection.as_mut().is_some_and(|selection| {
                            selection.accepts(&prepared, geometry(view.layout))
                        }) {
                            discard(id, prepared, cx);
                            view.retire(window, cx);
                            return;
                        }
                        // Keep the same overlay and transaction. Preparation may
                        // complete after app deactivation; never reactivate it.
                        let inline = if prepared.fixed_frame.is_some() {
                            super::super::inline_area::InlineArea::with_frame(
                                id,
                                view.layout,
                                prepared.fixed_frame,
                            )
                        } else {
                            super::super::inline_area::InlineArea::new(id, view.layout)
                        };
                        let presentation = super::super::CapturePresentation {
                            window_refresh: prepared.refresh,
                            ..Default::default()
                        };
                        view.editor = Some(cx.new(|cx| {
                            #[allow(unused_mut)]
                            let mut editor = super::super::ScreenshotEditor::new_with_inline(
                                prepared.session,
                                if prepared.fixed_frame.is_some() {
                                    "Window · frozen pixels"
                                } else {
                                    "Area · full-display pixels"
                                },
                                presentation,
                                Some(inline),
                                window,
                                cx,
                            );
                            #[cfg(any(debug_assertions, test))]
                            if let Some(fixture) = prepared.fixture {
                                editor
                                    .install_ai_fixture(fixture.authority, fixture.permit)
                                    .expect("validated generated-image authority");
                                editor.source = if prepared.fixed_frame.is_some() {
                                    "Window · supplied generated frame"
                                } else {
                                    "Area · supplied generated pixels"
                                };
                            }
                            editor
                        }));
                        discard(id, view.selection.take(), cx);
                        view.preparing_editor = false;
                        cx.notify();
                    }
                    Ok(None) => view.retire(window, cx),
                    Err(error) => {
                        report(id, error, cx);
                        view.retire(window, cx);
                    }
                }
            })
            .ok()
    });
    if updated.is_none() {
        mark_cancelled(id, cx);
    }
    if let Some(result) = result {
        mark_cancelled(id, cx);
        discard(id, result, cx);
    }
    finish_if_ready(id, cx);
}
impl Render for MainAreaSelector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = crate::theme::for_window(window).accent;
        let geometry = geometry(self.layout);
        let (sx, sy) = geometry
            .pixels_per_point()
            .expect("validated Area geometry");
        let selected = self
            .locked
            .or_else(|| self.selection.as_ref().and_then(|a| a.preview_rect()))
            .filter(|r| r.width > 0. && r.height > 0.);
        let border: f32 = if self.editor.is_some() {
            0.
        } else if self.locked.is_some() {
            2.5
        } else {
            2.
        };
        let bands = if self.editor.is_some() {
            [Rect::default(); 4]
        } else {
            selection::dim_bands(geometry.local_bounds(), selected).unwrap_or_default()
        };
        let viewport = self.viewport.clone();
        let width = geometry.cocoa_frame.width;
        let height = geometry.cocoa_frame.height;
        div()
            .id("area-main-display-selector")
            .relative()
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .cursor(CursorStyle::Crosshair)
            .track_focus(&self.focus)
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
                    move |bounds, _, _| viewport.set(bounds),
                    move |bounds, _, window, _| {
                        let at = |r: Rect| {
                            Bounds::new(
                                point(bounds.origin.x + px(r.x), bounds.origin.y + px(r.y)),
                                size(px(r.width), px(r.height)),
                            )
                        };
                        for band in bands {
                            if band.width > 0. && band.height > 0. {
                                window.paint_quad(fill(at(band), gpui::black().opacity(0.34)));
                            }
                        }
                        if let Some(rect) = selected.filter(|_| border > 0.) {
                            for edge in [
                                Rect::new(rect.x, rect.y, rect.width, border.min(rect.height)),
                                Rect::new(
                                    rect.x,
                                    rect.bottom() - border.min(rect.height),
                                    rect.width,
                                    border.min(rect.height),
                                ),
                                Rect::new(rect.x, rect.y, border.min(rect.width), rect.height),
                                Rect::new(
                                    rect.right() - border.min(rect.width),
                                    rect.y,
                                    border.min(rect.width),
                                    rect.height,
                                ),
                            ] {
                                window.paint_quad(fill(at(edge), accent));
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .when_some(selected.filter(|_| self.locked.is_none()), |d, rect| {
                d.child(
                    div()
                        .absolute()
                        .left(px(rect.x.min((width - 150.).max(0.))))
                        .top(px((rect.y - 26.).max(0.)))
                        .h(px(22.))
                        .px(px(6.))
                        .rounded(px(4.))
                        .bg(gpui::black().opacity(0.65))
                        .text_color(gpui::white())
                        .text_size(px(11.))
                        .child(format!(
                            "{} × {} px",
                            (rect.width * sx) as u32,
                            (rect.height * sy) as u32
                        )),
                )
            })
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if view.editor.is_none() && event.keystroke.key == "escape" {
                    view.retire(window, cx);
                }
            }))
            .when_some(self.editor.clone(), |root, editor| {
                root.child(div().absolute().inset_0().child(editor))
            })
            .when(self.quit_notice, |root| {
                // Absolute, noninteractive and nonoccluding: pointer geometry
                // and capture-phase selection release remain owned underneath.
                root.child(
                    div()
                        .absolute()
                        .top(px(12.))
                        .right(px(12.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .bg(gpui::black().opacity(0.8))
                        .text_color(gpui::white())
                        .text_size(px(12.))
                        .child("Finish or cancel capture, then Quit."),
                )
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod window_tests;
