//! Source-shaped Screenshot popup with local OCR and image-specific AI consent.
//! Ordinary provider upload remains gated before configuration or credential I/O.
use crate::{
    session::SessionJobs,
    theme::{self, Palette},
};
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::screenshot::{
    AnnotationKind, AnnotationStyle, AnnotationTool, MaskPattern, Point, Rect, RgbaColor,
    ScreenshotDocument, ScreenshotEditSession,
};
use gpui::{
    App, Bounds, ClipboardEntry, ClipboardItem, Context, Entity, FocusHandle, Image, ImageFormat,
    MouseButton, Pixels, Point as ViewPoint, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, canvas, div, img, point, prelude::*, px, size,
};
use std::{cell::Cell, rc::Rc, sync::Arc};

mod ai_ocr;
#[cfg(debug_assertions)]
mod area_capture;
mod area_transaction;
mod inline_area;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod main_area;
#[cfg(debug_assertions)]
mod scroll_capture;
mod window_refresh;
mod window_workflow;

const TOOLS: [(AnnotationTool, &str, &str); 9] = [
    (AnnotationTool::Select, "Select", "cursorarrow"),
    (AnnotationTool::Pen, "Pen", "pencil.tip"),
    (AnnotationTool::Arrow, "Arrow", "arrow.up.right"),
    (AnnotationTool::Rectangle, "Rectangle", "rectangle"),
    (AnnotationTool::Highlight, "Highlight", "highlighter"),
    (AnnotationTool::Text, "Text", "textformat"),
    (AnnotationTool::Crop, "Crop", "crop"),
    (AnnotationTool::Blur, "Mask", "checkerboard.rectangle"),
    (AnnotationTool::Eraser, "Eraser", "eraser"),
];
const ZOOM_STEPS: [f32; 8] = [0.25, 0.5, 0.75, 1., 1.5, 2., 3., 4.];
#[derive(Clone, Copy, Debug, PartialEq)]
enum Zoom {
    Fit,
    FitWidth,
    Scale(f32),
}
impl Zoom {
    fn scale(self, image: (f32, f32), viewport: (f32, f32)) -> f32 {
        if image.0 <= 0. || image.1 <= 0. || viewport.0 <= 0. || viewport.1 <= 0. {
            return 1.;
        }
        match self {
            Self::Fit => (viewport.0 / image.0).min(viewport.1 / image.1),
            Self::FitWidth => viewport.0 / image.0,
            Self::Scale(s) => {
                if s.is_finite() {
                    s.clamp(0.05, 8.)
                } else {
                    1.
                }
            }
        }
    }
    fn label(self, scale: f32) -> String {
        match self {
            Self::Fit => format!("{:.0}% · Fit", scale * 100.),
            Self::FitWidth => format!("{:.0}% · Fit Width", scale * 100.),
            Self::Scale(_) => format!("{:.0}%", scale * 100.),
        }
    }
}
fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    p: Palette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(30.))
        .px(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .border_1()
        .border_color(p.separator)
        .bg(p.surface)
        .text_size(px(12.))
        .cursor_pointer()
        .hover(move |s| s.bg(p.accent.opacity(0.09)))
        .child(label.into())
}
fn icon_button(
    id: impl Into<gpui::ElementId>,
    symbol: &'static str,
    selected: bool,
    p: Palette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(30.))
        .flex_none()
        .rounded(px(6.))
        .border_1()
        .border_color(if selected { p.accent } else { p.separator })
        .bg(if selected {
            p.accent.opacity(0.12)
        } else {
            p.surface
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(annotation_icon(symbol, p))
}
fn header(subtitle: String, p: Palette) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(44.))
        .flex_none()
        .child(
            div()
                .size(px(34.))
                .rounded(px(10.))
                .bg(p.teal.opacity(0.1))
                .flex()
                .items_center()
                .justify_center()
                .child(theme::tool_icon("screenshot", 20., p)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Screenshot"),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.secondary)
                        .child(subtitle),
                ),
        )
}

#[cfg(any(target_os = "macos", test))]
fn native_capture_should_focus(application_active: bool, requester_is_key: bool) -> bool {
    application_active && requester_is_key
}

fn caught_capture<T>(job: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
        .unwrap_or_else(|_| Err("The native capture worker could not complete.".into()))
}

#[derive(Default)]
struct NativeCaptureVisibility {
    busy: bool,
}
impl gpui::Global for NativeCaptureVisibility {}

struct CaptureChooser {
    busy: bool,
    status: String,
    jobs: SessionJobs,
    focus: FocusHandle,
}
#[cfg(debug_assertions)]
pub(crate) fn cancel_pending_area_fixture(cx: &mut App) {
    area_capture::cancel_pending_launch(cx);
}

/// New tool/launcher navigation retires visible and pending production selectors.
pub(crate) fn area_navigation_changed(cx: &mut App) {
    #[cfg(debug_assertions)]
    cancel_pending_area_fixture(cx);
    main_area::navigation_changed(cx);
    #[cfg(not(any(target_os = "macos", debug_assertions)))]
    let _ = cx;
}

pub fn open(cx: &mut App) {
    #[cfg(debug_assertions)]
    if std::env::var("BELLOBOX_AI_OCR_FIXTURE").as_deref() == Ok("1") {
        ai_ocr::open_fixture(cx);
        return;
    }
    #[cfg(debug_assertions)]
    if std::env::var_os("BELLOBOX_WINDOW_FIXTURE").is_some() {
        area_capture::open_window_fixture(cx);
        return;
    }
    #[cfg(debug_assertions)]
    if std::env::var_os("BELLOBOX_AREA_FIXTURE").is_some() {
        area_capture::open_fixture(cx);
        return;
    }
    #[cfg(debug_assertions)]
    if std::env::var_os("BELLOBOX_SCROLL_FIXTURE").is_some() {
        scroll_capture::open_fixture(cx);
        return;
    }
    let bounds = Bounds::centered(None, size(px(460.), px(380.)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(460.), px(380.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Screenshot — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                let mut chooser = CaptureChooser {
                    busy: false,
                    status: {
                        #[cfg(target_os = "macos")]
                        {
                            main_area::take_notice(cx).unwrap_or_default()
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            String::new()
                        }
                    },
                    jobs: SessionJobs::default(),
                    focus,
                };
                let weak = cx.entity().downgrade();
                window.on_window_should_close(cx, move |_window, cx| {
                    let _ = weak.update(cx, |this: &mut CaptureChooser, _| this.jobs.cancel());
                    main_area::requester_closed(_window.window_handle(), cx);
                    crate::shutdown::allow_close(_window, cx)
                });
                {
                    let requester = window.window_handle();
                    cx.on_release(move |_: &mut CaptureChooser, cx| {
                        main_area::requester_closed(requester, cx)
                    })
                    .detach();
                }
                #[cfg(debug_assertions)]
                if std::env::var_os("BELLOBOX_INLINE_AREA_FIXTURE").is_some()
                    || std::env::var_os("BELLOBOX_INLINE_WINDOW_FIXTURE").is_some()
                    || matches!(
                        std::env::var("BELLOBOX_AI_OCR_FIXTURE").as_deref(),
                        Ok("area" | "window")
                    )
                {
                    chooser.busy = true;
                    chooser.status = "Selecting from supplied synthetic pixels…".into();
                    let requester = window.window_handle();
                    cx.defer(move |cx| {
                        let _ = requester.update(cx, |root, window, cx| {
                            let result = if matches!(
                                std::env::var("BELLOBOX_AI_OCR_FIXTURE").as_deref(),
                                Ok("area" | "window")
                            ) {
                                main_area::begin_ai_fixture(
                                    window,
                                    cx,
                                    std::env::var("BELLOBOX_AI_OCR_FIXTURE").as_deref()
                                        == Ok("window"),
                                )
                            } else if std::env::var_os("BELLOBOX_INLINE_WINDOW_FIXTURE").is_some() {
                                main_area::begin_window_fixture(window, cx)
                            } else {
                                main_area::begin_fixture(window, cx)
                            };
                            if let Err(error) = result
                                && let Ok(chooser) = root.downcast::<CaptureChooser>()
                            {
                                chooser.update(cx, |chooser, cx| {
                                    chooser.busy = false;
                                    chooser.status = error;
                                    cx.notify();
                                });
                            }
                        });
                    });
                }
                if let Some(path) = std::env::var_os("BELLOBOX_SCREENSHOT_FILE") {
                    chooser.load_fixture(path.into(), window, cx);
                }
                chooser
            })
        },
    );
}
impl CaptureChooser {
    #[cfg(target_os = "macos")]
    fn capture_area(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        match main_area::begin(window, cx) {
            Ok(()) => {
                self.busy = true;
                self.status = "Freezing the main display…".into();
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }
    #[cfg(target_os = "macos")]
    fn capture_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        match main_area::begin_window(window, cx) {
            Ok(()) => {
                self.busy = true;
                self.status = "Freezing the main display…".into();
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }
    fn capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            if cx.default_global::<NativeCaptureVisibility>().busy {
                self.status = "Another screen capture is still finishing.".into();
                cx.notify();
                return;
            }
            cx.default_global::<NativeCaptureVisibility>().busy = true;
        }
        self.busy = true;
        self.status = "Capturing locally…".into();
        let token = self.jobs.begin();
        #[cfg(target_os = "macos")]
        let cancellation = self.jobs.cancellation();
        #[cfg(target_os = "macos")]
        let app_cx = cx.to_async();
        #[cfg(target_os = "macos")]
        cx.hide();
        #[cfg(not(target_os = "macos"))]
        window.minimize_window();
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            #[cfg(target_os = "macos")]
            {
                // Restore the hidden application even if a Rust worker panics.
                caught_capture(|| {
                    // Source default compositor delay; the whole application is hidden.
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    let cancel = bello_platform::native_capture::CaptureCancellation::from_flag(
                        cancellation,
                    );
                    let capture =
                        bello_platform::native_capture::capture_main_display_snapshot(cancel)
                            .map_err(|error| error.to_string())?;
                    prepare(capture.png)
                })
            }
            #[cfg(not(target_os = "macos"))]
            {
                // Preserve the existing portable helper path.
                std::thread::sleep(std::time::Duration::from_millis(250));
                let capture = bello_platform::Platform::new()
                    .capture_screenshot_snapshot()
                    .map_err(|e| e.to_string())?;
                prepare(capture.png)
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            #[cfg(target_os = "macos")]
            let (result, application_active) = {
                // Restore even if the requesting window/entity has been closed.
                // This never activates the app or steals focus from newer navigation.
                let restored = app_cx.update(|cx| {
                    cx.default_global::<NativeCaptureVisibility>().busy = false;
                    bello_platform::macos_native::unhide_application_without_activation()
                });
                match restored {
                    Ok(Ok(active)) => (result, active),
                    Ok(Err(error)) => (Err(error.to_string()), false),
                    Err(_) => return,
                }
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.jobs.accepts(token) {
                    return;
                }
                this.busy = false;
                #[cfg(not(target_os = "macos"))]
                window.activate_window();
                match result {
                    Ok(session) => {
                        #[cfg(target_os = "macos")]
                        {
                            // GPUI's macOS active_window is mainWindow, which may
                            // persist while another application has focus. Require
                            // actual AppKit activation AND this window's key state.
                            let focus = native_capture_should_focus(
                                application_active,
                                window.is_window_active(),
                            );
                            open_presented_session(
                                session,
                                "ScreenCaptureKit · macOS 14+",
                                CapturePresentation {
                                    open_unfocused: !focus,
                                    ..Default::default()
                                },
                                cx,
                            );
                        }
                        #[cfg(not(target_os = "macos"))]
                        open_session(session, "Screen capture", cx);
                        crate::shutdown::close_window(window, cx);
                    }
                    Err(e) => {
                        this.status = e;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let image = cx.read_from_clipboard().and_then(|item| {
            item.into_entries().find_map(|entry| match entry {
                ClipboardEntry::Image(image) => Some(image),
                _ => None,
            })
        });
        let Some(image) = image else {
            self.status = "The clipboard does not contain an image.".into();
            cx.notify();
            return;
        };
        if image.format != ImageFormat::Png {
            self.status = "This preview accepts PNG clipboard images only.".into();
            cx.notify();
            return;
        }
        self.busy = true;
        let token = self.jobs.begin();
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { prepare(image.bytes) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.jobs.accepts(token) {
                    return;
                }
                this.busy = false;
                match result {
                    Ok(session) => {
                        open_session(session, "Clipboard image", cx);
                        crate::shutdown::close_window(window, cx);
                    }
                    Err(e) => {
                        this.status = e;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}
impl Render for CaptureChooser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        #[cfg(target_os = "macos")]
        let area_available = main_area::PRODUCTION_AREA_ENABLED
            && !self.busy
            && bello_platform::macos_capture_overlay::main_display_overlay_layout().is_ok();
        #[cfg(not(target_os = "macos"))]
        let area_available = false;
        let area = button("area", "Area · main display only", p)
            .flex_1()
            .opacity(if area_available { 1. } else { 0.5 });
        #[cfg(target_os = "macos")]
        let area = area.when(area_available, |button| {
            button.on_click(cx.listener(|this, _, window, cx| this.capture_area(window, cx)))
        });
        let window_capture = button("window", "Window · main display only", p)
            .flex_1()
            .opacity(if main_area::PRODUCTION_WINDOW_ENABLED && !self.busy {
                1.
            } else {
                0.5
            });
        #[cfg(target_os = "macos")]
        let window_capture = window_capture.when(
            main_area::PRODUCTION_WINDOW_ENABLED && !self.busy,
            |button| {
                button.on_click(cx.listener(|this, _, window, cx| this.capture_window(window, cx)))
            },
        );
        div().size_full().p(px(18.)).flex().flex_col().gap(px(16.)).bg(p.bg).text_color(p.primary).font_family(theme::ui_font()).track_focus(&self.focus)
            .on_key_down(cx.listener(|this,e:&gpui::KeyDownEvent,window,_cx|{if e.keystroke.key=="escape"{this.jobs.cancel(); main_area::requester_closed(window.window_handle(), _cx); crate::shutdown::close_window(window, _cx);}}))
            .child(header("Capture and annotate".into(),p))
            .child(div().flex().flex_col().gap(px(10.)).child(div().flex().gap(px(10.)).child(area).child(window_capture)).child(div().flex().gap(px(10.)).child(button("screen",if self.busy{"Capturing…"}else{"Screen"},p).flex_1().on_click(cx.listener(Self::capture_click))).child(button("scroll","Scrolling · unavailable",p).flex_1().opacity(0.5))))
            .child(div().text_size(px(11.)).text_color(p.secondary).child("Screen captures the full virtual screen on Linux and the main display on macOS. Area, window and scrolling selection are not available yet."))
            .child(button("paste-image","Paste Image",p).on_click(cx.listener(|this,_,window,cx|this.paste(window,cx))))
            .child(div().text_size(px(11.)).text_color(p.danger).child(self.status.clone()))
            .child(div().flex_1())
            .child(div().text_size(px(10.)).text_color(p.secondary).child("Screenshots stay on this computer. Local OCR is available in the editor. AI OCR upload is not available in this preview."))
    }
}
impl CaptureChooser {
    fn capture_click(&mut self, _: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.capture(window, cx);
    }
}

fn prepare(png: Vec<u8>) -> Result<ScreenshotEditSession, String> {
    Ok(prepare_document(ScreenshotDocument::from_png(&png)?))
}

fn prepare_document(document: ScreenshotDocument) -> ScreenshotEditSession {
    prepare_session(ScreenshotEditSession::new(document))
}

fn prepare_session(mut session: ScreenshotEditSession) -> ScreenshotEditSession {
    // Fonts are optional local inputs, never downloaded or embedded without a license.
    #[cfg(target_os = "linux")]
    let paths = ["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"];
    #[cfg(target_os = "macos")]
    let paths = ["/System/Library/Fonts/Supplemental/Arial.ttf"];
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let paths: [&str; 0] = [];
    for path in paths {
        use std::io::Read;
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        let mut font = Vec::new();
        if file.take(16_000_001).read_to_end(&mut font).is_ok()
            && session.set_text_font(&font).is_ok()
        {
            break;
        }
    }
    session
}

#[derive(Clone)]
struct Gesture {
    tool: AnnotationTool,
    points: Vec<Point>,
    selected: Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorTarget {
    Stroke,
    Mask,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Zoom,
    MaskPattern,
    Image,
    LabelDelete(u64),
}
#[derive(Default)]
struct CapturePresentation {
    window_refresh: Option<window_refresh::Request>,
    open_unfocused: bool,
    scrolling: bool,
    frame_count: usize,
    notes: Vec<String>,
}

#[cfg(debug_assertions)]
struct PreparedScrollCapture {
    session: ScreenshotEditSession,
    presentation: CapturePresentation,
}

#[cfg(debug_assertions)]
fn prepare_scroll_result(
    result: bellobox_core::screenshot::scroll::ScrollResult,
) -> PreparedScrollCapture {
    PreparedScrollCapture {
        session: prepare_document(result.document),
        presentation: CapturePresentation {
            window_refresh: None,
            open_unfocused: false,
            scrolling: true,
            frame_count: result.frame_count,
            notes: result.notes.iter().map(ToString::to_string).collect(),
        },
    }
}

#[cfg(debug_assertions)]
fn open_scroll_result(result: PreparedScrollCapture, cx: &mut App) {
    open_presented_session(result.session, "Scrolling capture", result.presentation, cx);
}

struct ScreenshotEditor {
    inline: Option<inline_area::InlineArea>,
    window_refresh: Option<window_refresh::Host>,
    capture_presentation: CapturePresentation,
    shows_capture_notes: bool,
    shows_all_capture_notes: bool,
    session: ScreenshotEditSession,
    source: &'static str,
    tool: AnnotationTool,
    style: AnnotationStyle,
    mask_style: AnnotationStyle,
    eraser_width: f32,
    zoom: Zoom,
    scale: f32,
    preview_tiles: Vec<PreviewTile>,
    scroll: gpui::ScrollHandle,
    export_busy: bool,
    image_bounds: Rc<Cell<Bounds<Pixels>>>,
    preview_revision: u64,
    label_drag: Option<LabelDrag>,
    label_menu_position: ViewPoint<Pixels>,
    slider_bounds: Rc<Cell<Bounds<Pixels>>>,
    viewport_size: Rc<Cell<(f32, f32)>>,
    gesture: Option<Gesture>,
    selected: Option<u64>,
    status: String,
    error: bool,
    rendering: bool,
    jobs: SessionJobs,
    ocr_jobs: SessionJobs,
    ai_ocr: ai_ocr::Host,
    shows_ocr: bool,
    ocr_busy: bool,
    ocr_content: RevisionText,
    ocr: Entity<EditorView>,
    text_draft: Entity<EditorView>,
    color_input: Entity<EditorView>,
    color_target: Option<ColorTarget>,
    color_error: Option<String>,
    color_bounds: Rc<[Cell<Bounds<Pixels>>; 3]>,
    _color_subscription: gpui::Subscription,
    text_origin: Option<Point>,
    show_discard: bool,
    open_menu: Option<Menu>,
    menu_index: usize,
    focus: FocusHandle,
}
fn open_session(session: ScreenshotEditSession, source: &'static str, cx: &mut App) {
    open_presented_session(session, source, CapturePresentation::default(), cx);
}

fn open_presented_session(
    session: ScreenshotEditSession,
    source: &'static str,
    presentation: CapturePresentation,
    cx: &mut App,
) {
    let _ = try_open_presented_session(session, source, presentation, cx);
}
fn try_open_presented_session(
    session: ScreenshotEditSession,
    source: &'static str,
    presentation: CapturePresentation,
    cx: &mut App,
) -> Result<(), String> {
    let bounds = Bounds::centered(None, size(px(1040.), px(760.)), cx);
    cx.open_window(
        WindowOptions {
            focus: !presentation.open_unfocused,
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(640.), px(440.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Screenshot — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            cx.new(|cx| ScreenshotEditor::new(session, source, presentation, window, cx))
        },
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}
impl ScreenshotEditor {
    fn cancel_window_refresh(&mut self) {
        self.window_refresh = None;
    }
    fn new(
        session: ScreenshotEditSession,
        source: &'static str,
        presentation: CapturePresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_inline(session, source, presentation, None, window, cx)
    }
    fn new_with_inline(
        session: ScreenshotEditSession,
        source: &'static str,
        presentation: CapturePresentation,
        inline: Option<inline_area::InlineArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        // GPUI logical focus does not activate a native window. Preserve the
        // keyboard route when font preparation finishes while the app is inactive.
        window.focus(&focus);
        let ocr = cx.new(|cx| {
            let mut e = EditorView::new(String::new(), window, cx);
            e.set_read_only(true, cx);
            e.set_appearance(EditorAppearance::plain(), cx);
            e
        });
        let text_draft = cx.new(|cx| {
            let mut e = EditorView::new(String::new(), window, cx);
            e.set_appearance(EditorAppearance::plain(), cx);
            e
        });
        let color_input = cx.new(|cx| {
            let mut e = EditorView::new(String::new(), window, cx);
            e.set_appearance(EditorAppearance::plain(), cx);
            e.set_compact(true, cx);
            e
        });
        let color_subscription = cx.subscribe(&color_input, |this, _, event: &EditorEvent, cx| {
            if matches!(event, EditorEvent::Changed)
                && let Some(target) = this.color_target
            {
                match crate::screenshot_color::updated_from_hex(
                    this.picked_color(target),
                    this.color_input.read(cx).text(),
                ) {
                    Ok(color) => {
                        this.apply_custom_color(target, color);
                        this.color_error = None;
                    }
                    Err(error) => this.color_error = Some(error.into()),
                }
                cx.notify();
            }
        });
        let mut presentation = presentation;
        let window_refresh = presentation.window_refresh.take();
        let initial_zoom = if presentation.scrolling {
            Zoom::FitWidth
        } else {
            Zoom::Fit
        };
        let mut view = Self {
            inline,
            window_refresh: None,
            shows_capture_notes: !presentation.notes.is_empty(),
            shows_all_capture_notes: false,
            capture_presentation: presentation,
            session,
            source,
            tool: AnnotationTool::Select,
            style: AnnotationStyle::default(),
            mask_style: AnnotationStyle::redaction(),
            eraser_width: 24.,
            zoom: initial_zoom,
            scale: 1.,
            preview_tiles: Vec::new(),
            scroll: gpui::ScrollHandle::new(),
            export_busy: false,
            image_bounds: Rc::new(Cell::new(Bounds::default())),
            preview_revision: 0,
            label_drag: None,
            label_menu_position: point(px(0.), px(0.)),
            slider_bounds: Rc::new(Cell::new(Bounds::default())),
            viewport_size: Rc::new(Cell::new((0., 0.))),
            gesture: None,
            selected: None,
            status: String::new(),
            error: false,
            rendering: false,
            jobs: SessionJobs::default(),
            ocr_jobs: SessionJobs::default(),
            ai_ocr: ai_ocr::Host::new(cx),
            shows_ocr: false,
            ocr_busy: false,
            ocr_content: RevisionText::default(),
            ocr,
            text_draft,
            color_input,
            color_target: None,
            color_error: None,
            color_bounds: Rc::new(std::array::from_fn(|_| Cell::new(Bounds::default()))),
            _color_subscription: color_subscription,
            text_origin: None,
            show_discard: false,
            open_menu: None,
            menu_index: 0,
            focus,
        };
        view.watch_ai_owner(window, cx);
        cx.on_release(|this, cx| this.ai_ocr.retire(cx)).detach();
        let weak = cx.entity().downgrade();
        if view.inline.is_none() {
            window.on_window_should_close(cx, move |window, cx| {
                let allowed = weak
                    .update(cx, |this, cx| {
                        this.ai_ocr.retire(cx);
                        if this.export_busy {
                            return false;
                        }
                        if this.text_origin.is_some() {
                            this.cancel_active_text(window, cx);
                            return false;
                        }
                        this.finish_label_drag(cx);
                        if this.session.has_edits() {
                            this.show_discard = true;
                            cx.notify();
                            false
                        } else {
                            this.cancel_window_refresh();
                            true
                        }
                    })
                    .unwrap_or(true);
                allowed && crate::shutdown::allow_close(window, cx)
            });
        }
        view.render_preview(cx);
        if let Some(request) = window_refresh {
            view.start_window_refresh(request, cx);
        }
        view
    }
    fn capture_notes(&self, p: Palette, cx: &mut Context<Self>) -> gpui::Div {
        let notes = &self.capture_presentation.notes;
        let count = if self.shows_all_capture_notes {
            notes.len()
        } else {
            notes.len().min(2)
        };
        let mut list = div().id("capture-notes-list").flex().flex_col().gap(px(5.));
        for note in notes.iter().take(count) {
            list = list.child(
                div()
                    .text_size(px(12.))
                    .text_color(p.secondary)
                    .when(!self.shows_all_capture_notes, |note| {
                        note.max_h(px(36.)).overflow_hidden()
                    })
                    .child(note.clone()),
            );
        }
        if self.shows_all_capture_notes {
            list = list.max_h(px(132.)).overflow_y_scroll();
        }
        div()
            .flex()
            .gap(px(10.))
            .px(px(12.))
            .py(px(8.))
            .flex_none()
            .rounded(px(10.))
            .bg(p.accent.opacity(0.10))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!(
                                "Scrolling capture · {} frames",
                                self.capture_presentation.frame_count
                            )),
                    )
                    .child(list)
                    .when(notes.len() > 2, |column| {
                        column.child(
                            button(
                                "capture-notes-toggle",
                                if self.shows_all_capture_notes {
                                    "Show fewer".into()
                                } else {
                                    format!("Show all {} notes", notes.len())
                                },
                                p,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.shows_all_capture_notes = !this.shows_all_capture_notes;
                                cx.notify();
                            })),
                        )
                    }),
            )
            .child(
                button("dismiss-capture-notes", "Dismiss", p).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.shows_capture_notes = false;
                        cx.notify();
                    },
                )),
            )
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.invalidate_ocr_interaction(cx);
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.preview_tiles.clear();
        self.ocr_content.invalidate();
        self.ocr.update(cx, |e, cx| e.set_text(String::new(), cx));
        self.status.clear();
        self.error = false;
        self.render_preview(cx);
        cx.notify();
    }
    fn report(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => self.changed(cx),
            Err(e) => {
                self.status = e;
                self.error = true;
                cx.notify();
            }
        }
    }
    fn begin_inline_work(&self, cx: &mut App) -> Option<u64> {
        self.inline
            .as_ref()
            .and_then(|inline| main_area::begin_editor_work(inline.id, cx))
    }
    fn inline_current(&self, cx: &mut App) -> bool {
        self.inline
            .as_ref()
            .is_none_or(|inline| main_area::editor_current(inline.id, cx))
    }
    fn render_preview(&mut self, cx: &mut Context<Self>) {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return;
        }
        if self.rendering {
            return;
        }
        self.rendering = true;
        let token = self.jobs.begin();
        let revision = self.preview_revision;
        let snapshot = self.label_preview_snapshot().and_then(|snapshot| {
            if self.inline.is_some() {
                // Inline tiles stay in full-display coordinates, so a live crop
                // draft can reveal pixels without rerendering or moving annotations.
                let mut full = ScreenshotEditSession::new(snapshot);
                full.set_crop(None)?;
                Ok(full.render_snapshot())
            } else {
                Ok(snapshot)
            }
        });
        let task = cx
            .background_executor()
            .spawn(async move { snapshot?.render_preview_tiles() });
        let inline_work = self.begin_inline_work(cx);
        cx.spawn(async move |this, cx| {
            async {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if !this.inline_current(cx) {
                        this.cancel_inline_owner(cx);
                        return;
                    }
                    if !this.jobs.accepts(token) {
                        return;
                    }
                    this.rendering = false;
                    if this.preview_revision != revision {
                        this.render_preview(cx);
                        return;
                    }
                    match result {
                        Ok(tiles) => {
                            this.preview_tiles = tiles
                                .into_iter()
                                .map(|tile| PreviewTile {
                                    x: tile.x,
                                    y: tile.y,
                                    width: tile.width,
                                    height: tile.height,
                                    image: Arc::new(Image::from_bytes(ImageFormat::Png, tile.png)),
                                })
                                .collect();
                        }
                        Err(e) => {
                            this.preview_tiles.clear();
                            this.status = e;
                            this.error = true;
                        }
                    }
                    cx.notify();
                });
            }
            .await;
            let _ = cx.update(|cx| main_area::finish_editor_work(inline_work, cx));
        })
        .detach();
    }
    fn document_point(&self, position: ViewPoint<Pixels>) -> Point {
        let b = self.image_bounds.get();
        let visible = self.inline_visible_rect();
        let offset = (
            f32::from(position.x - b.origin.x),
            f32::from(position.y - b.origin.y),
        );
        let (x, y) = if let Some(inline) = self.inline.as_ref() {
            if inline.adjustable() {
                // Preserve source direct multiplication; a rounded reciprocal
                // can produce an extra outward-rounded crop pixel on Area.
                let (sx, sy) = inline.pixels_per_point();
                (offset.0 * sx, offset.1 * sy)
            } else {
                let (sx, sy) = inline.canvas_scales(visible);
                (offset.0 / sx, offset.1 / sy)
            }
        } else {
            (offset.0 / self.scale, offset.1 / self.scale)
        };
        Point {
            x: x.clamp(0., visible.width) + visible.x,
            y: y.clamp(0., visible.height) + visible.y,
        }
    }
    fn mouse_down(
        &mut self,
        e: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return;
        }
        if self.show_discard || self.export_busy || self.ai_ocr.modal() {
            return;
        }
        if self.text_origin.is_some() {
            self.commit_text(cx);
            window.focus(&self.focus);
            return;
        }
        window.focus(&self.focus);
        let point = self.document_point(e.position);
        if self.tool == AnnotationTool::Text {
            if !self.session.has_text_font() {
                self.status = "A supported local font is required for text annotations.".into();
                self.error = true;
                cx.notify();
                return;
            }
            self.invalidate_ocr_interaction(cx);
            self.text_origin = Some(point);
            self.text_draft.update(cx, |e, cx| {
                e.set_text(String::new(), cx);
                e.focus(window);
            });
            cx.notify();
            return;
        }
        self.selected = None;
        if self.tool == AnnotationTool::Select {
            if self
                .inline
                .as_ref()
                .is_some_and(|inline| inline.adjustable())
            {
                self.begin_selection_adjustment(None, e.position, cx);
            }
            cx.notify();
            return;
        }
        self.invalidate_ocr_interaction(cx);
        self.gesture = Some(Gesture {
            tool: self.tool,
            points: vec![point],
            selected: self.selected,
        });
        cx.notify();
    }
    fn mouse_move(&mut self, e: &gpui::MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return;
        }
        if self.update_selection_adjustment(e.position, false, cx) {
            return;
        }
        let point = self.document_point(e.position);
        if self.label_drag.is_some() {
            self.update_label_drag(point, cx);
            return;
        }
        if let Some(gesture) = self.gesture.as_mut() {
            if gesture.points.len() >= 20_000 {
                self.status =
                    "This stroke reached the 20,000-point limit; release to finish.".into();
                return;
            }
            if matches!(gesture.tool, AnnotationTool::Pen | AnnotationTool::Eraser) {
                let last = gesture.points.last().unwrap();
                if (point.x - last.x).hypot(point.y - last.y) < 0.75 {
                    return;
                }
                gesture.points.push(point);
            } else if gesture.points.len() == 1 {
                gesture.points.push(point);
            } else {
                gesture.points[1] = point;
            }
            cx.notify();
        }
    }
    fn mouse_up(&mut self, e: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return;
        }
        if self.update_selection_adjustment(e.position, true, cx) {
            return;
        }
        let endpoint = self.document_point(e.position);
        if self.label_drag.is_some() {
            self.update_label_drag(endpoint, cx);
            self.finish_label_drag(cx);
            return;
        }
        let Some(mut gesture) = self.gesture.take() else {
            return;
        };
        complete_gesture(&mut gesture, endpoint);
        let first = gesture.points[0];
        let last = *gesture.points.last().unwrap();
        let rect = Rect {
            x: first.x.min(last.x),
            y: first.y.min(last.y),
            width: (last.x - first.x).abs(),
            height: (last.y - first.y).abs(),
        };
        let result = match gesture.tool {
            AnnotationTool::Select => {
                if let Some(id) = gesture.selected {
                    self.session
                        .move_annotation(id, last.x - first.x, last.y - first.y)
                        .map(|_| ())
                } else {
                    Ok(())
                }
            }
            AnnotationTool::Crop => {
                if rect.width >= 1. && rect.height >= 1. {
                    self.session.set_crop(Some(rect)).map(|_| ())
                } else {
                    Ok(())
                }
            }
            AnnotationTool::Eraser => self
                .session
                .erase_stroke(gesture.points, self.eraser_width)
                .map(|_| ()),
            tool => {
                let (kind, style) = match tool {
                    AnnotationTool::Pen => (
                        AnnotationKind::Freehand {
                            points: gesture.points,
                        },
                        self.style,
                    ),
                    AnnotationTool::Arrow => (
                        AnnotationKind::Arrow {
                            start: first,
                            end: last,
                        },
                        self.style,
                    ),
                    AnnotationTool::Rectangle => (AnnotationKind::Rectangle(rect), self.style),
                    AnnotationTool::Highlight => (
                        AnnotationKind::Highlight(rect),
                        AnnotationStyle::highlight(),
                    ),
                    AnnotationTool::Blur => (AnnotationKind::Blur(rect), self.mask_style),
                    _ => return,
                };
                self.session.add_annotation(kind, style).map(|_| ())
            }
        };
        self.report(result, cx);
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ai_ocr.retire(cx);
        if let Some(inline) = self.inline.as_ref() {
            main_area::cancel(inline.id, cx);
            return;
        }
        if self.text_origin.is_some() {
            self.cancel_active_text(window, cx);
            return;
        }
        self.finish_label_drag(cx);
        if self.session.has_edits() {
            self.show_discard = true;
            cx.notify();
        } else {
            self.cancel_window_refresh();
            crate::shutdown::close_window(window, cx);
        }
    }
    fn prepare_output_snapshot(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<bellobox_core::screenshot::RenderSnapshot> {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return None;
        }
        if self.gesture.is_some() {
            // Crop, masks, pen and eraser are still visual drafts. Never export
            // older wider/unredacted pixels behind the active pointer gesture.
            self.status =
                "Release the pointer to finish the edit before exporting or reading text.".into();
            self.error = false;
            cx.notify();
            return None;
        }
        if !self.commit_selection_adjustment(cx) {
            return None;
        }
        self.finish_label_drag(cx);
        self.commit_text(cx).then(|| self.session.render_snapshot())
    }
    fn copy(&mut self, finish: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_busy {
            return;
        }
        let Some(snapshot) = self.prepare_output_snapshot(cx) else {
            return;
        };
        self.export_busy = true;
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { snapshot.render_png() });
        let inline_work = self.begin_inline_work(cx);
        let app = cx.to_async();
        cx.spawn_in(window, async move |this, cx| {
            async {
                let result = task.await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if !this.inline_current(cx) {
                        this.cancel_inline_owner(cx);
                        return;
                    }
                    this.export_busy = false;
                    match result {
                        Ok(png) => {
                            cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
                                ImageFormat::Png,
                                png,
                            )));
                            this.status = "Copied image.".into();
                            this.error = false;
                            if finish {
                                this.cancel_window_refresh();
                                if this.inline.is_some() {
                                    this.cancel_inline_owner(cx);
                                } else {
                                    crate::shutdown::close_window(window, cx);
                                }
                            }
                        }
                        Err(error) => {
                            this.status = error;
                            this.error = true;
                        }
                    }
                    cx.notify();
                });
            }
            .await;
            let _ = app.update(|cx| main_area::finish_editor_work(inline_work, cx));
        })
        .detach();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.export_busy {
            return;
        }
        let Some(snapshot) = self.prepare_output_snapshot(cx) else {
            return;
        };
        self.export_busy = true;
        cx.notify();
        let cancellation = self
            .inline
            .as_ref()
            .and_then(|inline| main_area::editor_cancellation(inline.id, cx));
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let name = format!("BelloBox-Screenshot-{timestamp}.png");
        let dialog = cx.prompt_for_new_path(
            &std::env::current_dir().unwrap_or_else(|_| ".".into()),
            Some(&name),
        );
        let inline_work = self.begin_inline_work(cx);
        cx.spawn(async move |this, cx| {
            async {
                let path = match dialog.await {
                    Ok(Ok(Some(path))) => path,
                    Ok(Ok(None)) => {
                        let _ = this.update(cx, |this, cx| {
                            if !this.inline_current(cx) {
                                this.cancel_inline_owner(cx);
                                return;
                            }
                            this.export_busy = false;
                            cx.notify();
                        });
                        return;
                    }
                    _ => {
                        let _ = this.update(cx, |this, cx| {
                            if !this.inline_current(cx) {
                                this.cancel_inline_owner(cx);
                                return;
                            }
                            this.export_busy = false;
                            this.status = "Could not open the save dialog.".into();
                            this.error = true;
                            cx.notify();
                        });
                        return;
                    }
                };
                if !this
                    .update(cx, |this, cx| {
                        let current = this.inline_current(cx);
                        if !current {
                            this.cancel_inline_owner(cx);
                        }
                        current
                    })
                    .unwrap_or(false)
                {
                    return;
                }
                let task = cx.background_executor().spawn(async move {
                    if !path
                        .extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
                    {
                        return Err("Choose a PNG filename.".into());
                    }
                    let png = snapshot.render_png()?;
                    if cancellation
                        .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
                    {
                        return Err("Screenshot export was cancelled.".into());
                    }
                    bello_platform::Platform::new()
                        .save_png_bytes(&path, &png)
                        .map_err(|e| e.to_string())?;
                    Ok::<_, String>(())
                });
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if !this.inline_current(cx) {
                        this.cancel_inline_owner(cx);
                        return;
                    }
                    this.export_busy = false;
                    match result {
                        Ok(()) => {
                            this.status = "Saved PNG.".into();
                            this.error = false;
                        }
                        Err(e) => {
                            this.status = e;
                            this.error = true;
                        }
                    }
                    cx.notify();
                });
            }
            .await;
            let _ = cx.update(|cx| main_area::finish_editor_work(inline_work, cx));
        })
        .detach();
    }
    fn read_text(&mut self, cx: &mut Context<Self>) {
        if self.ocr_busy || self.ai_ocr_busy() || self.ai_ocr.modal() {
            return;
        }
        let Some(snapshot) = self.prepare_output_snapshot(cx) else {
            return;
        };
        let revision = self.session.revision();
        let token = OcrTicket {
            job: self.ocr_jobs.begin(),
            revision,
        };
        self.ocr_busy = true;
        self.status = "Reading the cropped, masked image locally…".into();
        self.error = false;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let image = snapshot.render_for_external_ocr_png()?;
            bello_platform::Platform::new()
                .recognize_image_bytes(&image, None)
                .map_err(|e| e.to_string())
        });
        let inline_work = self.begin_inline_work(cx);
        cx.spawn(async move |this, cx| {
            async {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if !this.inline_current(cx) {
                        this.cancel_inline_owner(cx);
                        return;
                    }
                    if !token.accepts(&this.ocr_jobs, this.session.revision()) {
                        return;
                    }
                    this.ocr_busy = false;
                    match result {
                        Ok(text) => {
                            this.clear_ai_result();
                            this.ocr_content = RevisionText {
                                revision: Some(revision),
                                text: text.clone(),
                            };
                            this.ocr.update(cx, |e, cx| e.set_text(text, cx));
                            this.status = "Local OCR complete. Nothing was uploaded.".into();
                        }
                        Err(e) => {
                            this.status = e;
                            this.error = true;
                        }
                    }
                    cx.notify();
                });
            }
            .await;
            let _ = cx.update(|cx| main_area::finish_editor_work(inline_work, cx));
        })
        .detach();
    }
    fn key_down(&mut self, e: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_modal_key(e, window, cx) {
            return;
        }
        let key = e.keystroke.key.as_str();
        if self.export_busy {
            cx.stop_propagation();
            return;
        }
        if self.show_discard {
            if key == "escape" {
                self.show_discard = false;
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if self.open_menu.is_some() {
            match key {
                "escape" => self.open_menu = None,
                "up" => self.menu_index = self.menu_index.saturating_sub(1),
                "down" => self.menu_index = (self.menu_index + 1).min(self.menu_labels().len() - 1),
                "enter" => self.choose_menu(self.menu_index, cx),
                _ => {}
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }

        let primary = e.keystroke.modifiers.platform || e.keystroke.modifiers.control;
        if self.inline.is_some() && key == "enter" && self.text_origin.is_none() {
            self.copy(true, window, cx);
            cx.stop_propagation();
            return;
        }
        if self.inline.is_some() && key == "escape" {
            self.close(window, cx);
            cx.stop_propagation();
            return;
        }
        if key == "escape" {
            if let Some(drag) = self.label_drag.take() {
                if drag.id.is_none() {
                    self.text_origin = Some(drag.origin);
                }
                self.preview_revision = self.preview_revision.wrapping_add(1);
                self.render_preview(cx);
                cx.notify();
                return;
            }
            if self.text_origin.take().is_some()
                || self.gesture.take().is_some()
                || self.show_discard
            {
                self.show_discard = false;
                cx.notify();
            } else {
                self.close(window, cx);
            }
            return;
        }
        if primary && key == "z" {
            self.apply_history(e.keystroke.modifiers.shift, cx);
            cx.stop_propagation();
        } else if primary && key == "s" {
            self.save(cx);
            cx.stop_propagation();
        } else if primary && e.keystroke.modifiers.shift && key == "c" {
            self.copy(false, window, cx);
            cx.stop_propagation();
        } else if primary && key == "0" {
            self.zoom = Zoom::Scale(1.);
            cx.notify();
        } else if primary && key == "9" {
            self.zoom = Zoom::Fit;
            cx.notify();
        } else if primary && key == "-" {
            self.zoom_step(false, cx);
        } else if primary && (key == "=" || key == "+") {
            self.zoom_step(true, cx);
        } else if self.inline.is_none() && primary && e.keystroke.modifiers.alt && key == "o" {
            self.shows_ocr = !self.shows_ocr;
            cx.notify();
        } else if primary
            && e.keystroke.modifiers.alt
            && let Ok(n) = key.parse::<usize>()
            && let Some((tool, _, _)) = n.checked_sub(1).and_then(|i| TOOLS.get(i))
        {
            if !self.commit_text(cx) {
                return;
            }
            self.tool = *tool;
            cx.notify();
        }
    }
    fn zoom_step(&mut self, up: bool, cx: &mut Context<Self>) {
        self.zoom = Zoom::Scale(if up {
            ZOOM_STEPS
                .into_iter()
                .find(|s| *s > self.scale + 0.001)
                .unwrap_or(4.)
        } else {
            ZOOM_STEPS
                .into_iter()
                .rev()
                .find(|s| *s < self.scale - 0.001)
                .unwrap_or(0.25)
        });
        cx.notify();
    }
}
impl ScreenshotEditor {
    fn toolbar(&self, compact: bool, p: Palette, cx: &mut Context<Self>) -> gpui::Div {
        let tools = div()
            .flex()
            .gap(px(4.))
            .children(
                TOOLS
                    .into_iter()
                    .enumerate()
                    .map(|(index, (tool, _, symbol))| {
                        icon_button(("annotation-tool", index), symbol, self.tool == tool, p)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if !this.commit_text(cx) {
                                    return;
                                }
                                this.tool = tool;
                                this.gesture = None;
                                window.focus(&this.focus);
                                cx.notify();
                            }))
                    }),
            );
        let history = div()
            .flex()
            .gap(px(4.))
            .child(
                icon_button("undo", "arrow.uturn.backward", false, p)
                    .opacity(if self.session.can_undo() { 1. } else { 0.35 })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.apply_history(false, cx);
                    })),
            )
            .child(
                icon_button("redo", "arrow.uturn.forward", false, p)
                    .opacity(if self.session.can_redo() { 1. } else { 0.35 })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.apply_history(true, cx);
                    })),
            );
        let controls = self.style_controls(p, cx);
        let options = icon_button("image-options", "ellipsis", false, p).on_click(cx.listener(
            |this, event: &gpui::ClickEvent, _, cx| {
                this.record_inline_menu_anchor(event.position());
                this.toggle_menu(Menu::Image, cx);
            },
        ));
        if compact {
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(tools)
                        .child(div().flex_1())
                        .child(history),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(controls)
                        .child(options),
                )
        } else {
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(tools)
                .child(div().h(px(24.)).w(px(1.)).bg(p.separator))
                .child(controls)
                .child(options)
                .child(div().flex_1())
                .child(history)
        }
    }
    fn style_controls(&self, p: Palette, cx: &mut Context<Self>) -> gpui::Div {
        let mut row = div().flex().items_center().gap(px(5.)).text_size(px(10.));
        match self.tool {
            AnnotationTool::Blur => {
                let colors = [
                    RgbaColor::new(0.16, 0.16, 0.16, 1.),
                    RgbaColor::new(0.02, 0.02, 0.02, 1.),
                    RgbaColor::new(0.98, 0.97, 0.95, 1.),
                    RgbaColor::new(0.89, 0.46, 0.15, 1.),
                    RgbaColor::new(0.06, 0.40, 0.43, 1.),
                    RgbaColor::new(0.46, 0.26, 0.70, 1.),
                ];
                row = row.children(colors.into_iter().enumerate().map(|(i, color)| {
                    div()
                        .id(("mask-fill", i))
                        .size(px(18.))
                        .rounded_full()
                        .bg(gpui::Rgba {
                            r: color.red,
                            g: color.green,
                            b: color.blue,
                            a: 1.,
                        })
                        .border_1()
                        .border_color(if self.mask_style.fill_color == Some(color) {
                            p.accent
                        } else {
                            p.border
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.mask_style.fill_color = Some(color);
                            cx.notify();
                        }))
                }));
                row = row.child(self.color_well(ColorTarget::Mask, p, cx));
                let symbol = match self.mask_style.mask_pattern {
                    MaskPattern::Solid => "mask-solid",
                    MaskPattern::Stripes => "mask-stripes",
                    MaskPattern::Dots => "mask-dots",
                };
                row = row.child(
                    icon_button("mask-pattern", symbol, false, p)
                        .size(px(24.))
                        .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                            this.record_inline_menu_anchor(event.position());
                            this.toggle_menu(Menu::MaskPattern, cx);
                        })),
                );
            }
            AnnotationTool::Pen
            | AnnotationTool::Arrow
            | AnnotationTool::Rectangle
            | AnnotationTool::Text => {
                row = row.child(self.color_well(ColorTarget::Stroke, p, cx));
                let is_text = self.tool == AnnotationTool::Text;
                let value = if is_text {
                    self.style.font_size
                } else {
                    self.style.line_width
                };
                if is_text {
                    row = row
                        .child(
                            button("width-down", "−", p)
                                .px(px(4.))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if is_text {
                                        this.style.font_size = (this.style.font_size - 2.).max(10.);
                                    } else {
                                        this.style.line_width =
                                            (this.style.line_width - 1.).max(1.);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(format!("{value:.0} px"))
                        .child(button("width-up", "+", p).px(px(4.)).on_click(cx.listener(
                            move |this, _, _, cx| {
                                if is_text {
                                    this.style.font_size = (this.style.font_size + 2.).min(72.);
                                } else {
                                    this.style.line_width = (this.style.line_width + 1.).min(12.);
                                }
                                cx.notify();
                            },
                        )));
                } else {
                    row = row
                        .child(self.style_slider(p, cx))
                        .child(format!("{value:.0} px"));
                }
            }
            AnnotationTool::Eraser => {
                row = row
                    .child(self.style_slider(p, cx))
                    .child(format!("{:.0} px", self.eraser_width));
            }
            tool => {
                row = row.w(px(145.)).text_color(p.secondary).child(match tool {
                    AnnotationTool::Select => "Select and move annotations",
                    AnnotationTool::Crop => "Drag to crop the image",
                    _ => "Drag to highlight a region",
                })
            }
        }
        row
    }
    fn canvas(
        &mut self,
        available: (f32, f32),
        p: Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let measured = self.viewport_size.get();
        let available = if measured.0 > 0. && measured.1 > 0. {
            measured
        } else {
            available
        };
        let visible = self.session.document().visible_rect();
        self.scale = self.zoom.scale((visible.width, visible.height), available);
        let scale = self.scale;
        let (width, height) = (visible.width * scale, visible.height * scale);
        let viewport_size = self.viewport_size.clone();
        let weak = cx.entity().downgrade();
        let measure = canvas(
            move |bounds, _, cx| {
                let next = (
                    f32::from(bounds.size.width).max(1.),
                    f32::from(bounds.size.height).max(1.),
                );
                if viewport_size.get() != next {
                    viewport_size.set(next);
                    let weak = weak.clone();
                    cx.defer(move |cx| {
                        let _ = weak.update(cx, |_, cx| cx.notify());
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        div()
            .id("screenshot-viewport")
            .relative()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.separator)
            .bg(p.well)
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
            .child(measure)
            .child(
                div()
                    .w(px(width.max(available.0)))
                    .h(px(height.max(available.1)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.annotation_surface(visible, (scale, scale), available, p, cx)),
            )
    }
    fn annotation_surface(
        &mut self,
        visible: Rect,
        scales: (f32, f32),
        available: (f32, f32),
        p: Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (scale, scale_y) = scales;
        let handles_pointer = self
            .inline
            .as_ref()
            .is_none_or(|inline| inline.adjustable());
        let tile_origin = if self.inline.is_some() {
            Point::new(visible.x, visible.y)
        } else {
            Point::default()
        };
        if self.text_origin.is_some() {
            let style = self.style;
            self.text_draft.update(cx, |editor, cx| {
                let mut appearance = editor.appearance().clone();
                let color = p.primary;
                let font_size = (style.font_size * scale).clamp(6., 256.);
                if appearance.font_size != font_size || appearance.text != color {
                    appearance.font_size = font_size;
                    appearance.line_height = font_size * 1.4;
                    appearance.text = color;
                    editor.set_appearance(appearance, cx);
                    editor.set_compact(true, cx);
                }
            });
        }
        let (width, height) = (visible.width * scale, visible.height * scale_y);
        let bounds_cell = self.image_bounds.clone();
        let gesture = self.gesture.clone();
        let style = self.style;
        let eraser = self.eraser_width;
        let overlay = canvas(
            move |bounds, _, _| {
                bounds_cell.set(bounds);
            },
            move |bounds, _, window, _| {
                if let Some(g) = &gesture {
                    paint_gesture(
                        g,
                        visible,
                        (scale, scale_y),
                        bounds,
                        (style.line_width, eraser),
                        p,
                        window,
                    );
                }
            },
        )
        .absolute()
        .size_full();
        div()
            .id("annotation-canvas")
            .relative()
            .flex_none()
            .w(px(width))
            .h(px(height))
            // Clip full-base preview tiles at the fitted image, not its text
            // controls: source inline text/drag handles may extend into padding.
            .child(
                div().absolute().size_full().overflow_hidden().children(
                    self.preview_tiles
                        .iter()
                        .filter(|tile| {
                            self.inline.is_some()
                                || tile_is_visible(
                                    tile,
                                    scale,
                                    (
                                        width.max(available.0) - width,
                                        height.max(available.1) - height,
                                    ),
                                    self.scroll.offset(),
                                    available,
                                )
                        })
                        .map(|tile| {
                            div()
                                .absolute()
                                .left(px((tile.x as f32 - tile_origin.x) * scale))
                                .top(px((tile.y as f32 - tile_origin.y) * scale_y))
                                .w(px(tile.width as f32 * scale))
                                .h(px(tile.height as f32 * scale_y))
                                .child(img(tile.image.clone()).size_full())
                        }),
                ),
            )
            .child(overlay)
            .children(self.label_overlays(visible, (scale, scale_y), p, cx))
            .when_some(self.text_origin, |d, origin| {
                d.child(
                    div()
                        .id("inline-annotation-text")
                        .absolute()
                        .left(px((origin.x - visible.x) * scale))
                        .top(px((origin.y - visible.y) * scale_y))
                        .w(px((260. * scale).max(120.)))
                        .h(px(
                            ((self.style.font_size + 16.).max(34.) * scale_y).max(30.)
                        ))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.accent)
                        .rounded(px(4.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .capture_key_down(cx.listener(
                            |this, e: &gpui::KeyDownEvent, window, cx| {
                                if this.text_draft.read(cx).has_marked_text() {
                                    return;
                                }
                                if e.keystroke.key == "enter" {
                                    this.commit_text(cx);
                                    window.focus(&this.focus);
                                    cx.stop_propagation();
                                } else if e.keystroke.key == "escape" {
                                    this.cancel_active_text(window, cx);
                                    cx.stop_propagation();
                                }
                            },
                        ))
                        .child(self.text_draft.clone())
                        .child(
                            div()
                                .id("active-label-drag-handle")
                                .absolute()
                                .left(px(-10.))
                                .top(px(-10.))
                                .size(px(22.))
                                .rounded_full()
                                .bg(p.accent_fill)
                                .border_1()
                                .border_color(gpui::white().opacity(0.45))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_move()
                                .child(annotation_icon("move", p))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, e: &gpui::MouseDownEvent, _, cx| {
                                        this.begin_label_drag(None, e.position, cx);
                                        cx.stop_propagation();
                                    }),
                                ),
                        ),
                )
            })
            .when(handles_pointer, |surface| {
                surface
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
                    .on_mouse_move(cx.listener(Self::mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            })
    }
    fn reader(&self, p: Palette, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let current = self.reader_text(false).is_some();
        let markdown = self.reader_text(true).is_some();
        div().id("ocr-reader-panel").min_h_0().overflow_y_scroll().w(px(285.)).flex_none().flex().flex_col().gap(px(8.)).p(px(10.))
            .bg(p.surface).rounded(px(10.)).border_1().border_color(p.separator)
            .child(div().text_size(px(14.)).font_weight(gpui::FontWeight::SEMIBOLD).child("Text Reader"))
            .child(div().flex().gap(px(6.))
                .child(button("read-local", if self.ocr_busy { "Cancel" } else if cfg!(target_os = "macos") { "Read on Mac" } else { "Read Locally" }, p)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.ocr_busy { this.ocr_jobs.cancel(); this.ocr_busy = false; this.status = "Local OCR canceled; its late result will be ignored.".into(); cx.notify(); }
                        else { this.read_text(cx); }
                    })))
                .child(button("ai-ocr", if self.ai_ocr_busy() { "Cancel AI OCR" } else { "AI OCR…" }, p)
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.ai_ocr_busy() { this.cancel_ai_ocr(window, cx); } else { this.request_ai_ocr(window, cx); }
                    }))))
            .child(self.fixture_controls(p, cx))
            .child(div().text_size(px(10.)).text_color(p.secondary)
                .child("AI upload requires a preview and explicit approval. Local hints and structured regions are unavailable."))
            .when(markdown, |d| d.child(div().flex().gap(px(6.))
                .child(button("ocr-show-text", "Text", p).on_click(cx.listener(|this, _, _, cx| this.refresh_reader_mode(false, cx))))
                .child(button("ocr-show-markdown", "Markdown (literal)", p).on_click(cx.listener(|this, _, _, cx| this.refresh_reader_mode(true, cx))))))
            .when(!self.reader_warnings().is_empty(), |d| d.child(div().id("ocr-result-warnings").debug_selector(|| "ocr-result-warnings".into()).flex_none().max_h(px(60.)).overflow_y_scroll().text_size(px(10.)).text_color(p.secondary).child(self.reader_warnings())))
            .child(div().flex_1().min_h(px(100.)).rounded(px(8.)).bg(p.well).p(px(8.)).child(self.ocr.clone()))
            .child(div().flex().flex_wrap().gap(px(6.))
                .child(button("copy-ocr", "Copy Text", p).opacity(if current { 1. } else { 0.4 })
                    .on_click(cx.listener(|this, _, _, cx| this.copy_reader(false, cx))))
                .child(button("copy-markdown", "Copy Markdown", p).opacity(if markdown { 1. } else { 0.4 })
                    .on_click(cx.listener(|this, _, _, cx| this.copy_reader(true, cx))))
                .child(button("save-ocr", "Save…", p).opacity(if current { 1. } else { 0.4 })
                    .on_click(cx.listener(|this, _, _, cx| this.save_reader(cx)))))
    }
    fn footer(&self, compact: bool, p: Palette, cx: &mut Context<Self>) -> gpui::Div {
        let navigation = div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(
                button("text-reader", "Text Reader", p).on_click(cx.listener(|this, _, _, cx| {
                    this.shows_ocr = !this.shows_ocr;
                    cx.notify();
                })),
            )
            .child(
                button("zoom-out", "−", p)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_step(false, cx))),
            )
            .child(
                button("zoom", format!("{} ▾", self.zoom.label(self.scale)), p).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.toggle_menu(Menu::Zoom, cx);
                    }),
                ),
            )
            .child(
                button("zoom-in", "+", p)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_step(true, cx))),
            );
        let exports = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(button("copy-image", "Copy Image", p).on_click(cx.listener(
                |this, _, window, cx| {
                    this.copy(false, window, cx);
                },
            )))
            .child(
                button("save-png", "Save PNG…", p)
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            )
            .child(
                button("copy-finish", "Copy & Finish", p)
                    .bg(p.accent_fill)
                    .text_color(gpui::white())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.copy(true, window, cx);
                    })),
            );
        let status = div()
            .flex_1()
            .min_w(px(0.))
            .text_size(px(10.))
            .text_color(if self.error { p.danger } else { p.secondary })
            .child(if self.rendering {
                "Rendering current edits…".to_string()
            } else {
                self.status.clone()
            });
        if compact {
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(navigation)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(status)
                        .child(exports),
                )
        } else {
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(navigation)
                .child(status)
                .child(exports)
        }
    }
}
impl Render for ScreenshotEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.check_ai_result_authority(cx);
        if self.inline.is_some() {
            return self.render_inline(window, cx).into_any_element();
        }
        let render_started = std::time::Instant::now();
        let p = theme::for_window(window);
        let viewport = window.viewport_size();
        let compact = f32::from(viewport.width) < 760.;
        let visible = self.session.document().visible_rect();
        for editor in [&self.ocr, &self.color_input] {
            editor.update(cx, |e, cx| {
                let mut appearance = e.appearance().clone();
                if appearance.text != p.primary {
                    appearance.text = p.primary;
                    appearance.caret = p.accent;
                    appearance.selection = p.accent.opacity(0.18);
                    e.set_appearance(appearance, cx);
                }
            });
        }
        let available = (
            (f32::from(viewport.width) - 22. - if self.shows_ocr { 297. } else { 0. }).max(1.),
            (f32::from(viewport.height) - if compact { 180. } else { 142. }).max(1.),
        );
        let body = div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .gap(px(12.))
            .child(self.canvas(available, p, cx))
            .when(self.shows_ocr, |d| d.child(self.reader(p, cx)));
        let mut root = div()
            .size_full()
            .relative()
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(theme::ui_font())
            .track_focus(&self.focus)
            .capture_any_mouse_down(
                cx.listener(|this, _, _, cx| this.check_ai_result_authority(cx)),
            )
            .capture_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, window, cx| {
                this.check_ai_result_authority(cx);
                if this.ai_modal_key(e, window, cx) {
                    return;
                }
                if this.color_target.is_some()
                    && !this.color_input.read(cx).has_marked_text()
                    && matches!(e.keystroke.key.as_str(), "escape" | "enter")
                {
                    this.color_target = None;
                    this.color_error = None;
                    window_focus_after_color(this, window, cx);
                    cx.stop_propagation();
                    return;
                }
                if this.color_target.is_some()
                    && (e.keystroke.modifiers.alt
                        || ((e.keystroke.modifiers.platform || e.keystroke.modifiers.control)
                            && !["a", "c", "v", "x", "z"].contains(&e.keystroke.key.as_str())))
                {
                    cx.stop_propagation();
                    return;
                }
                if this.export_busy || this.show_discard {
                    if e.keystroke.key == "escape" && !this.export_busy {
                        this.show_discard = false;
                        cx.notify();
                    }
                    cx.stop_propagation();
                }
            }))
            .on_key_down(cx.listener(Self::key_down))
            .child(
                header(
                    format!(
                        "{} · {:.0} × {:.0} px",
                        if self.capture_presentation.scrolling {
                            format!(
                                "{} · {} frames",
                                self.source, self.capture_presentation.frame_count
                            )
                        } else {
                            self.source.to_owned()
                        },
                        visible.width,
                        visible.height
                    ),
                    p,
                )
                .child(div().flex_1())
                .child(
                    icon_button("minimize", "minus", false, p)
                        .on_click(cx.listener(|_, _, window, _| window.minimize_window())),
                )
                .child(
                    icon_button("close-screenshot", "xmark", false, p)
                        .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                ),
            )
            .when(self.shows_capture_notes, |root| {
                root.child(self.capture_notes(p, cx))
            })
            .child(self.toolbar(compact, p, cx))
            .child(body)
            .child(self.footer(compact, p, cx));
        if self.open_menu.is_some() {
            root = root.child(self.menu(p, compact, cx));
        }
        if self.color_target.is_some() {
            root = root.child(self.color_panel(p, cx));
        }

        if self.export_busy {
            root = root.child(
                div()
                    .id("screenshot-export-lock")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(p.bg.opacity(0.78))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(self.export_caption()),
            );
        }
        if self.show_discard {
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .id("screenshot-discard-confirmation")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .bg(gpui::black().opacity(0.3))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(390.))
                            .p(px(18.))
                            .rounded(px(12.))
                            .bg(p.surface)
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Discard screenshot edits?"),
                            )
                            .child(div().text_size(px(12.)).child(
                                "Your screenshot annotations and crop changes will be lost.",
                            ))
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap(px(8.))
                                    .child(button("keep-editing", "Keep Editing", p).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.show_discard = false;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(
                                        button("discard", "Discard", p)
                                            .text_color(p.danger)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.cancel_window_refresh();
                                                crate::shutdown::close_window(window, cx)
                                            })),
                                    ),
                            ),
                    ),
            );
        }
        if self.ai_ocr.modal() {
            root = root.child(self.ai_confirmation(window, p, cx));
        }
        crate::desktop::perf("render_cpu", render_started.elapsed().as_micros());
        root.into_any_element()
    }
}

fn paint_gesture(
    g: &Gesture,
    visible: Rect,
    scales: (f32, f32),
    bounds: Bounds<Pixels>,
    brushes: (f32, f32),
    p: Palette,
    window: &mut Window,
) {
    let (scale, scale_y) = scales;
    let transform = |v: Point| {
        point(
            bounds.origin.x + px((v.x - visible.x) * scale),
            bounds.origin.y + px((v.y - visible.y) * scale_y),
        )
    };
    let mut path = gpui::PathBuilder::stroke(px(if g.tool == AnnotationTool::Eraser {
        brushes.1 * scale
    } else {
        (brushes.0 * scale).max(1.)
    }));
    let first = g.points[0];
    let last = *g.points.last().unwrap();
    path.move_to(transform(first));
    if matches!(
        g.tool,
        AnnotationTool::Rectangle
            | AnnotationTool::Highlight
            | AnnotationTool::Crop
            | AnnotationTool::Blur
    ) {
        for v in [
            Point {
                x: last.x,
                y: first.y,
            },
            last,
            Point {
                x: first.x,
                y: last.y,
            },
            first,
        ] {
            path.line_to(transform(v));
        }
    } else {
        for v in &g.points[1..] {
            path.line_to(transform(*v));
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(
            path,
            p.accent.opacity(if g.tool == AnnotationTool::Eraser {
                0.25
            } else {
                0.9
            }),
        );
    }
}
fn annotation_icon(symbol: &'static str, p: Palette) -> gpui::AnyElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let lines: Vec<Vec<(f32, f32)>> = match symbol {
                "mask-solid" => vec![
                    vec![(3., 3.), (21., 3.), (21., 21.), (3., 21.), (3., 3.)],
                    vec![(3., 5.), (21., 5.)],
                    vec![(3., 8.), (21., 8.)],
                    vec![(3., 11.), (21., 11.)],
                    vec![(3., 14.), (21., 14.)],
                    vec![(3., 17.), (21., 17.)],
                    vec![(3., 20.), (21., 20.)],
                ],
                "mask-stripes" => vec![
                    vec![(3., 8.), (8., 3.)],
                    vec![(3., 16.), (16., 3.)],
                    vec![(7., 21.), (21., 7.)],
                    vec![(15., 21.), (21., 15.)],
                ],
                "mask-dots" => (0..3)
                    .flat_map(|y| {
                        (0..3).map(move |x| {
                            let x = 4. + x as f32 * 7.;
                            let y = 4. + y as f32 * 7.;
                            vec![(x, y), (x + 2., y), (x + 2., y + 2.), (x, y + 2.), (x, y)]
                        })
                    })
                    .collect(),
                "move" => vec![
                    vec![(12., 2.), (12., 22.)],
                    vec![(2., 12.), (22., 12.)],
                    vec![(8., 6.), (12., 2.), (16., 6.)],
                    vec![(8., 18.), (12., 22.), (16., 18.)],
                    vec![(6., 8.), (2., 12.), (6., 16.)],
                    vec![(18., 8.), (22., 12.), (18., 16.)],
                ],
                "cursorarrow" => vec![vec![
                    (4., 2.),
                    (4., 19.),
                    (9., 14.),
                    (13., 21.),
                    (16., 19.),
                    (12., 12.),
                    (19., 12.),
                    (4., 2.),
                ]],
                "pencil.tip" | "highlighter" => vec![
                    vec![
                        (4., 20.),
                        (6., 13.),
                        (17., 2.),
                        (22., 7.),
                        (11., 18.),
                        (4., 20.),
                    ],
                    vec![(6., 13.), (11., 18.)],
                ],
                "arrow.up.right" => vec![
                    vec![(3., 21.), (21., 3.), (11., 3.)],
                    vec![(21., 3.), (21., 13.)],
                ],
                "rectangle" => vec![vec![(3., 5.), (21., 5.), (21., 19.), (3., 19.), (3., 5.)]],
                "textformat" => vec![
                    vec![(2., 19.), (8., 4.), (14., 19.)],
                    vec![(5., 13.), (11., 13.)],
                    vec![
                        (16., 11.),
                        (21., 11.),
                        (21., 20.),
                        (16., 20.),
                        (16., 15.),
                        (21., 15.),
                    ],
                ],
                "crop" => vec![
                    vec![(6., 2.), (6., 18.), (22., 18.)],
                    vec![(2., 6.), (18., 6.), (18., 22.)],
                ],
                "checkerboard.rectangle" => vec![
                    vec![(3., 3.), (21., 3.), (21., 21.), (3., 21.), (3., 3.)],
                    vec![(9., 3.), (9., 21.)],
                    vec![(15., 3.), (15., 21.)],
                    vec![(3., 9.), (21., 9.)],
                    vec![(3., 15.), (21., 15.)],
                ],
                "eraser" => vec![
                    vec![
                        (3., 15.),
                        (15., 3.),
                        (22., 10.),
                        (11., 21.),
                        (8., 21.),
                        (3., 15.),
                    ],
                    vec![(8., 10.), (16., 17.)],
                    vec![(10., 21.), (22., 21.)],
                ],
                "arrow.uturn.backward" => vec![
                    vec![(8., 4.), (3., 9.), (8., 14.)],
                    vec![
                        (3., 9.),
                        (16., 9.),
                        (20., 12.),
                        (20., 17.),
                        (17., 20.),
                        (11., 20.),
                    ],
                ],
                "arrow.uturn.forward" => vec![
                    vec![(16., 4.), (21., 9.), (16., 14.)],
                    vec![
                        (21., 9.),
                        (8., 9.),
                        (4., 12.),
                        (4., 17.),
                        (7., 20.),
                        (13., 20.),
                    ],
                ],
                "ellipsis" => vec![
                    vec![(4., 11.), (5., 11.), (5., 13.), (4., 13.), (4., 11.)],
                    vec![(11., 11.), (12., 11.), (12., 13.), (11., 13.), (11., 11.)],
                    vec![(18., 11.), (19., 11.), (19., 13.), (18., 13.), (18., 11.)],
                ],
                "doc.on.doc" => vec![
                    vec![(8., 7.), (21., 7.), (21., 22.), (8., 22.), (8., 7.)],
                    vec![(16., 7.), (16., 2.), (3., 2.), (3., 17.), (8., 17.)],
                ],
                "square.and.arrow.down" => vec![
                    vec![(4., 13.), (4., 21.), (20., 21.), (20., 13.)],
                    vec![(12., 2.), (12., 16.)],
                    vec![(7., 11.), (12., 16.), (17., 11.)],
                ],
                "checkmark" => vec![vec![(3., 12.), (9., 19.), (21., 4.)]],
                "minus" => vec![vec![(5., 12.), (19., 12.)]],
                _ => vec![vec![(6., 6.), (18., 18.)], vec![(6., 18.), (18., 6.)]],
            };
            for line in lines {
                let mut path = gpui::PathBuilder::stroke(px(1.35));
                for (i, (x, y)) in line.into_iter().enumerate() {
                    let v = point(
                        bounds.origin.x + px(x / 24. * 18.),
                        bounds.origin.y + px(y / 24. * 18.),
                    );
                    if i == 0 {
                        path.move_to(v);
                    } else {
                        path.line_to(v);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, p.primary);
                }
            }
        },
    )
    .size(px(18.))
    .into_any_element()
}

impl ScreenshotEditor {
    fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.open_menu = if self.open_menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
        self.menu_index = 0;
        cx.notify();
    }
    fn menu_labels(&self) -> Vec<String> {
        match self.open_menu {
            Some(Menu::Zoom) => ["Fit".into(), "Fit Width".into(), "Actual Size".into()]
                .into_iter()
                .chain(ZOOM_STEPS.map(|s| format!("{:.0}%", s * 100.)))
                .collect(),
            Some(Menu::MaskPattern) => MaskPattern::ALL
                .map(|pattern| pattern.label().into())
                .into(),
            Some(Menu::Image) => vec!["Reset Crop".into()],
            Some(Menu::LabelDelete(_)) => vec!["Delete Label".into()],
            None => Vec::new(),
        }
    }
    fn choose_menu(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.open_menu {
            Some(Menu::Zoom) => {
                self.zoom = match index {
                    0 => Zoom::Fit,
                    1 => Zoom::FitWidth,
                    2 => Zoom::Scale(1.),
                    n => Zoom::Scale(*ZOOM_STEPS.get(n - 3).unwrap_or(&1.)),
                }
            }
            Some(Menu::MaskPattern) => {
                if let Some(pattern) = MaskPattern::ALL.get(index) {
                    self.mask_style.mask_pattern = *pattern;
                }
            }
            Some(Menu::Image) => {
                let result = self.session.set_crop(None).map(|_| ());
                self.report(result, cx);
            }
            Some(Menu::LabelDelete(id)) => {
                let result = self.session.remove_annotation(id).map(|_| ());
                self.report(result, cx);
            }
            None => {}
        }
        self.open_menu = None;
        cx.notify();
    }
    fn menu(&self, p: Palette, compact: bool, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let menu = self.open_menu.unwrap();
        let inline_origin = self.inline_panel_origin(
            180.,
            self.menu_labels().len() as f32 * 30. + 10.,
            matches!(menu, Menu::LabelDelete(_)),
        );
        let card = div()
            .id("screenshot-menu")
            .absolute()
            .left(px(if let Some(origin) = inline_origin {
                origin.x
            } else if matches!(menu, Menu::LabelDelete(_)) {
                f32::from(self.label_menu_position.x)
            } else if menu == Menu::Zoom {
                130.
            } else if compact {
                12.
            } else {
                470.
            }))
            .when(menu == Menu::Zoom && inline_origin.is_none(), |d| {
                d.bottom(px(if compact { 78. } else { 46. }))
            })
            .when(menu != Menu::Zoom || inline_origin.is_some(), |d| {
                d.top(px(if let Some(origin) = inline_origin {
                    origin.y
                } else if matches!(menu, Menu::LabelDelete(_)) {
                    f32::from(self.label_menu_position.y)
                } else if compact {
                    126.
                } else {
                    90.
                }))
            })
            .w(px(180.))
            .p(px(4.))
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .rounded(px(8.))
            .shadow_md()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(
                self.menu_labels()
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| {
                        button(("screenshot-menu-item", i), label, p)
                            .w_full()
                            .justify_start()
                            .border_0()
                            .when(i == self.menu_index, |d| d.bg(p.accent.opacity(0.12)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose_menu(i, cx);
                                cx.stop_propagation();
                            }))
                    }),
            );
        div()
            .id("screenshot-menu-scrim")
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.open_menu = None;
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .child(card)
    }
}

impl ScreenshotEditor {
    fn set_slider(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let bounds = self.slider_bounds.get();
        let fraction = ((f32::from(x - bounds.origin.x) - 4.)
            / (f32::from(bounds.size.width) - 8.).max(1.))
        .clamp(0., 1.);
        if self.tool == AnnotationTool::Eraser {
            self.eraser_width = 6. + (fraction * 45.).round() * 2.;
        } else {
            self.style.line_width = 1. + (fraction * 11.).round();
        }
        cx.notify();
    }
    fn style_slider(&self, p: Palette, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let bounds = self.slider_bounds.clone();
        let fraction = if self.tool == AnnotationTool::Eraser {
            (self.eraser_width - 6.) / 90.
        } else {
            (self.style.line_width - 1.) / 11.
        };
        div()
            .id("annotation-width-slider")
            .w(px(if self.tool == AnnotationTool::Eraser {
                108.
            } else {
                70.
            }))
            .h(px(24.))
            .flex_none()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &gpui::MouseDownEvent, _, cx| {
                    this.set_slider(e.position.x, cx)
                }),
            )
            .on_mouse_move(cx.listener(|this, e: &gpui::MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) {
                    this.set_slider(e.position.x, cx);
                }
            }))
            .child(
                canvas(
                    move |b, _, _| bounds.set(b),
                    move |b, _, window, _| {
                        let y = b.origin.y + px(11.);
                        let width = f32::from(b.size.width) - 8.;
                        let left = b.origin.x + px(4.);
                        let current = left + px(width * fraction);
                        window.paint_quad(
                            gpui::fill(
                                Bounds::new(point(left, y), size(px(width), px(3.))),
                                p.border,
                            )
                            .corner_radii(px(2.)),
                        );
                        window.paint_quad(
                            gpui::fill(
                                Bounds::new(point(left, y), size(px(width * fraction), px(3.))),
                                p.accent,
                            )
                            .corner_radii(px(2.)),
                        );
                        window.paint_quad(
                            gpui::fill(
                                Bounds::new(
                                    point(current - px(5.), y - px(4.)),
                                    size(px(11.), px(11.)),
                                ),
                                p.surface,
                            )
                            .corner_radii(px(6.)),
                        );
                    },
                )
                .size_full(),
            )
    }
}

impl ScreenshotEditor {
    fn commit_text(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(origin) = self.text_origin else {
            return true;
        };
        let text = self.text_draft.read(cx).text().to_string();
        if text.trim().is_empty() {
            self.text_origin = None;
            cx.notify();
            return true;
        }
        match self.session.add_annotation(
            AnnotationKind::Text {
                text,
                origin,
                max_width: 260.,
            },
            self.style,
        ) {
            Ok(_) => {
                self.text_origin = None;
                self.changed(cx);
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
}

fn complete_gesture(gesture: &mut Gesture, endpoint: Point) {
    if gesture.points.last() == Some(&endpoint) {
        return;
    }
    if (matches!(gesture.tool, AnnotationTool::Pen | AnnotationTool::Eraser)
        && gesture.points.len() < 20_000)
        || gesture.points.len() == 1
    {
        gesture.points.push(endpoint);
    } else {
        *gesture.points.last_mut().unwrap() = endpoint;
    }
}
impl CaptureChooser {
    fn load_fixture(
        &mut self,
        path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.status = "Loading the explicit local PNG fixture…".into();
        let token = self.jobs.begin();
        let task = cx.background_executor().spawn(async move {
            use std::io::Read;
            let file = std::fs::File::open(path)
                .map_err(|_| "Could not open the selected PNG fixture.")?;
            if !file
                .metadata()
                .map_err(|_| "Could not inspect the PNG fixture.")?
                .is_file()
            {
                return Err("Select a PNG file.".into());
            }
            let mut png = Vec::new();
            file.take(bellobox_core::screenshot::MAX_PNG_BYTES as u64 + 1)
                .read_to_end(&mut png)
                .map_err(|_| "Could not read the PNG fixture.")?;
            prepare(png)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.jobs.accepts(token) {
                    return;
                }
                this.busy = false;
                match result {
                    Ok(session) => {
                        open_session(session, "Local PNG fixture", cx);
                        crate::shutdown::close_window(window, cx);
                    }
                    Err(error) => {
                        this.status = error;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}
struct PreviewTile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    image: Arc<Image>,
}
fn tile_is_visible(
    tile: &PreviewTile,
    scale: f32,
    padding: (f32, f32),
    scroll: ViewPoint<Pixels>,
    viewport: (f32, f32),
) -> bool {
    let x = tile.x as f32 * scale + padding.0 / 2. + f32::from(scroll.x);
    let y = tile.y as f32 * scale + padding.1 / 2. + f32::from(scroll.y);
    x < viewport.0
        && y < viewport.1
        && x + tile.width as f32 * scale > 0.
        && y + tile.height as f32 * scale > 0.
}

#[derive(Default)]
struct RevisionText {
    revision: Option<u64>,
    text: String,
}
impl RevisionText {
    fn invalidate(&mut self) {
        self.revision = None;
        self.text.clear();
    }
    fn current(&self, revision: u64) -> Option<&str> {
        (self.revision == Some(revision)).then_some(self.text.as_str())
    }
}
#[derive(Clone, Copy)]
struct OcrTicket {
    job: crate::session::JobToken,
    revision: u64,
}
impl OcrTicket {
    fn accepts(self, jobs: &SessionJobs, current: u64) -> bool {
        self.revision == current && jobs.accepts(self.job)
    }
}

fn window_focus_after_color(
    this: &mut ScreenshotEditor,
    window: &mut Window,
    cx: &mut Context<ScreenshotEditor>,
) {
    if this.text_origin.is_some() {
        this.text_draft.read(cx).focus(window);
    } else {
        window.focus(&this.focus);
    }
    cx.notify();
}
impl ScreenshotEditor {
    fn picked_color(&self, target: ColorTarget) -> RgbaColor {
        match target {
            ColorTarget::Stroke => self.style.stroke_color,
            ColorTarget::Mask => self.mask_style.mask_fill(),
        }
    }
    fn apply_custom_color(&mut self, target: ColorTarget, mut color: RgbaColor) {
        color.alpha = 1.;
        match target {
            ColorTarget::Stroke => self.style.stroke_color = color,
            ColorTarget::Mask => self.mask_style.fill_color = Some(color),
        }
    }
    fn open_color(&mut self, target: ColorTarget, window: &mut Window, cx: &mut Context<Self>) {
        self.open_menu = None;
        self.color_target = Some(target);
        self.color_error = None;
        let text = crate::screenshot_color::hex(self.picked_color(target));
        self.color_input.update(cx, |e, cx| {
            e.set_text(text, cx);
            e.focus(window);
        });
        cx.notify();
    }
    fn color_well(
        &self,
        target: ColorTarget,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let color = self.picked_color(target);
        div()
            .id(if target == ColorTarget::Mask {
                "custom-mask-color"
            } else {
                "custom-stroke-color"
            })
            .w(px(34.))
            .h(px(24.))
            .p(px(3.))
            .border_1()
            .border_color(p.border)
            .rounded(px(5.))
            .bg(p.surface)
            .cursor_pointer()
            .child(div().size_full().rounded(px(2.)).bg(gpui::Rgba {
                r: color.red,
                g: color.green,
                b: color.blue,
                a: 1.,
            }))
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    this.record_inline_menu_anchor(event.position());
                    this.open_color(target, window, cx);
                }),
            )
    }
    fn update_color_slider(&mut self, index: usize, x: Pixels, cx: &mut Context<Self>) {
        let Some(target) = self.color_target else {
            return;
        };
        let bounds = self.color_bounds[index].get();
        let value = ((f32::from(x - bounds.origin.x) - 5.)
            / (f32::from(bounds.size.width) - 10.).max(1.))
        .clamp(0., 1.)
            * 255.;
        let color = crate::screenshot_color::set_channel(self.picked_color(target), index, value);
        self.apply_custom_color(target, color);
        self.color_input.update(cx, |e, cx| {
            e.set_text(crate::screenshot_color::hex(color), cx)
        });
        cx.notify();
    }
    fn color_panel(&self, p: Palette, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let color = self.picked_color(self.color_target.unwrap());
        let channels = color.rgba8();
        let inline_origin = self.inline_panel_origin(300., 260., false);
        let rows = (0..3).map(|index| {
            let bounds = self.color_bounds.clone();
            let value = channels[index];
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(42.))
                        .text_size(px(11.))
                        .child(["Red", "Green", "Blue"][index]),
                )
                .child(
                    div()
                        .id(("color-channel", index))
                        .flex_1()
                        .h(px(24.))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, e: &gpui::MouseDownEvent, _, cx| {
                                this.update_color_slider(index, e.position.x, cx)
                            }),
                        )
                        .on_mouse_move(cx.listener(move |this, e: &gpui::MouseMoveEvent, _, cx| {
                            if e.pressed_button == Some(MouseButton::Left) {
                                this.update_color_slider(index, e.position.x, cx);
                            }
                        }))
                        .child(
                            canvas(
                                move |b, _, _| bounds[index].set(b),
                                move |b, _, window, _| {
                                    let width = f32::from(b.size.width) - 10.;
                                    let left = b.origin.x + px(5.);
                                    let y = b.origin.y + px(11.);
                                    let fraction = value as f32 / 255.;
                                    window.paint_quad(
                                        gpui::fill(
                                            Bounds::new(point(left, y), size(px(width), px(3.))),
                                            p.border,
                                        )
                                        .corner_radii(px(2.)),
                                    );
                                    window.paint_quad(
                                        gpui::fill(
                                            Bounds::new(
                                                point(left, y),
                                                size(px(width * fraction), px(3.)),
                                            ),
                                            p.accent,
                                        )
                                        .corner_radii(px(2.)),
                                    );
                                    window.paint_quad(
                                        gpui::fill(
                                            Bounds::new(
                                                point(
                                                    left + px(width * fraction) - px(5.),
                                                    y - px(4.),
                                                ),
                                                size(px(11.), px(11.)),
                                            ),
                                            p.accent,
                                        )
                                        .corner_radii(px(6.)),
                                    );
                                },
                            )
                            .size_full(),
                        ),
                )
                .child(div().w(px(26.)).text_size(px(11.)).child(value.to_string()))
        });
        div()
            .id("color-panel-scrim")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.color_target = None;
                    this.color_error = None;
                    window_focus_after_color(this, window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .id("screenshot-color-panel")
                    .absolute()
                    .left(px(inline_origin.map_or(40., |origin| origin.x)))
                    .top(px(inline_origin.map_or(90., |origin| origin.y)))
                    .w(px(300.))
                    .p(px(14.))
                    .rounded(px(12.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Colors"),
                            )
                            .child(icon_button("close-color", "xmark", false, p).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.color_target = None;
                                    this.color_error = None;
                                    window_focus_after_color(this, window, cx);
                                }),
                            )),
                    )
                    .children(rows)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(div().text_size(px(11.)).child("Hex"))
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(30.))
                                    .bg(p.well)
                                    .border_1()
                                    .border_color(p.separator)
                                    .rounded(px(5.))
                                    .child(self.color_input.clone()),
                            ),
                    )
                    .when_some(self.color_error.clone(), |d, error| {
                        d.child(div().text_size(px(10.)).text_color(p.danger).child(error))
                    })
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(p.secondary)
                            .child("Stroke, text and mask colors are always opaque."),
                    ),
            )
    }
}

#[derive(Clone, Copy)]
struct LabelDrag {
    id: Option<u64>,
    start: Point,
    origin: Point,
    current: Point,
    max_width: f32,
    font_size: f32,
    moved: bool,
}
impl ScreenshotEditor {
    fn begin_label_drag(
        &mut self,
        id: Option<u64>,
        position: ViewPoint<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.export_busy || self.show_discard || self.ai_ocr.modal() {
            return;
        }
        if id.is_some() && !self.commit_text(cx) {
            return;
        }
        let (origin, max_width, font_size) = if let Some(id) = id {
            let Some(annotation) = self
                .session
                .document()
                .annotations()
                .iter()
                .find(|a| a.id == id)
            else {
                return;
            };
            let AnnotationKind::Text {
                origin, max_width, ..
            } = annotation.kind
            else {
                return;
            };
            (origin, max_width, annotation.style.font_size)
        } else {
            let Some(origin) = self.text_origin else {
                return;
            };
            (origin, 260., self.style.font_size)
        };
        self.invalidate_ocr_interaction(cx);
        self.gesture = None;
        self.selected = id;
        self.label_drag = Some(LabelDrag {
            id,
            start: self.document_point(position),
            origin,
            current: origin,
            max_width,
            font_size,
            moved: false,
        });
        cx.notify();
    }
    fn update_label_drag(&mut self, point: Point, cx: &mut Context<Self>) {
        let Some(mut drag) = self.label_drag else {
            return;
        };
        if !drag.moved && !label_drag_exceeds_threshold(drag.start, point, self.scale) {
            return;
        }
        drag.moved = true;
        let proposed = Point::new(
            drag.origin.x + point.x - drag.start.x,
            drag.origin.y + point.y - drag.start.y,
        );
        let Ok(current) = bellobox_core::screenshot::selection::clamped_document_text_origin(
            proposed,
            drag.max_width,
            drag.font_size,
            self.session.document().visible_rect(),
        ) else {
            return;
        };
        if current == drag.current {
            return;
        }
        drag.current = current;
        self.label_drag = Some(drag);
        if drag.id.is_none() {
            self.text_origin = Some(current);
        } else {
            self.ocr_jobs.cancel();
            self.ocr_busy = false;
            self.ocr_content.invalidate();
            self.ocr.update(cx, |e, cx| e.set_text(String::new(), cx));
            self.preview_revision = self.preview_revision.wrapping_add(1);
            self.render_preview(cx);
        }
        cx.notify();
    }
    fn finish_label_drag(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.label_drag.take() else {
            return;
        };
        if let Some(id) = drag.id
            && drag.current != drag.origin
        {
            let result = self
                .session
                .move_annotation(
                    id,
                    drag.current.x - drag.origin.x,
                    drag.current.y - drag.origin.y,
                )
                .map(|_| ());
            self.report(result, cx);
        } else {
            cx.notify();
        }
    }
    fn label_preview_snapshot(&self) -> Result<bellobox_core::screenshot::RenderSnapshot, String> {
        transient_label_snapshot(self.session.render_snapshot(), self.label_drag)
    }
    fn label_overlays(
        &self,
        visible: Rect,
        scales: (f32, f32),
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Stateful<gpui::Div>> {
        let (scale, scale_y) = scales;
        if !matches!(self.tool, AnnotationTool::Select | AnnotationTool::Text) {
            return Vec::new();
        }
        self.session
            .document()
            .annotations()
            .iter()
            .filter_map(|annotation| {
                let AnnotationKind::Text {
                    origin, max_width, ..
                } = annotation.kind
                else {
                    return None;
                };
                let origin = self
                    .label_drag
                    .filter(|d| d.id == Some(annotation.id))
                    .map_or(origin, |d| d.current);
                let frame = bellobox_core::screenshot::selection::visible_text_label_frame(
                    origin,
                    max_width,
                    annotation.style.font_size,
                    Point::new(visible.x, visible.y),
                )
                .ok()?;
                let id = annotation.id;
                Some(
                    div()
                        .id(("committed-text-label", id as usize))
                        .absolute()
                        .left(px((frame.x + frame.width / 2.) * scale
                            - (frame.width * scale).max(44.) / 2.))
                        .top(px((frame.y + frame.height / 2.) * scale_y
                            - (frame.height * scale_y).max(30.) / 2.))
                        .w(px((frame.width * scale).max(44.)))
                        .h(px((frame.height * scale_y).max(30.)))
                        .border_1()
                        .border_color(p.accent.opacity(
                            if self.label_drag.is_some_and(|d| d.id == Some(id)) {
                                0.95
                            } else {
                                0.42
                            },
                        ))
                        .rounded(px(7.))
                        .cursor_move()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                                this.begin_label_drag(Some(id), e.position, cx);
                                window.focus(&this.focus);
                                cx.stop_propagation();
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                                this.open_menu = Some(Menu::LabelDelete(id));
                                this.menu_index = 0;
                                let size = window.viewport_size();
                                this.label_menu_position = point(
                                    e.position.x.min(size.width - px(190.)).max(px(0.)),
                                    e.position.y.min(size.height - px(50.)).max(px(0.)),
                                );
                                window.focus(&this.focus);
                                cx.notify();
                                cx.stop_propagation();
                            }),
                        ),
                )
            })
            .collect()
    }
}

fn transient_label_snapshot(
    snapshot: bellobox_core::screenshot::RenderSnapshot,
    drag: Option<LabelDrag>,
) -> Result<bellobox_core::screenshot::RenderSnapshot, String> {
    if let Some(drag) = drag
        && let Some(id) = drag.id
        && drag.current != drag.origin
    {
        if !snapshot
            .annotations()
            .iter()
            .any(|a| a.id == id && matches!(a.kind, AnnotationKind::Text { .. }))
        {
            return Err("Only text labels can be dragged in the screenshot editor.".into());
        }
        let mut preview = ScreenshotEditSession::new(snapshot);
        preview.move_annotation(
            id,
            drag.current.x - drag.origin.x,
            drag.current.y - drag.origin.y,
        )?;
        Ok(preview.render_snapshot())
    } else {
        Ok(snapshot)
    }
}
impl ScreenshotEditor {
    fn cancel_active_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.text_origin = None;
        self.label_drag = None;
        self.text_draft
            .update(cx, |e, cx| e.set_text(String::new(), cx));
        window.focus(&self.focus);
        cx.notify();
    }
    fn apply_history(&mut self, redo: bool, cx: &mut Context<Self>) {
        if self
            .inline
            .as_mut()
            .is_some_and(|inline| inline.adjustment.take().is_some())
        {
            // The draft has not entered history yet. Undo cancels that visible
            // gesture before touching the previous committed annotation/edit.
            cx.notify();
            return;
        }
        self.finish_label_drag(cx);
        if !self.commit_text(cx) {
            return;
        }
        if redo {
            self.session.redo();
        } else {
            self.session.undo();
        }
        self.changed(cx);
    }
}

fn label_drag_exceeds_threshold(start: Point, current: Point, scale: f32) -> bool {
    scale.is_finite()
        && scale > 0.
        && (current.x - start.x).hypot(current.y - start.y) * scale >= 2.
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_capture_focus_requires_active_application_and_requester_key_window() {
        assert!(super::native_capture_should_focus(true, true));
        assert!(!super::native_capture_should_focus(false, true));
        assert!(!super::native_capture_should_focus(true, false));
        assert!(!super::native_capture_should_focus(false, false));
    }

    #[test]
    fn native_capture_worker_preserves_results_and_contains_panics() {
        assert_eq!(super::caught_capture(|| Ok(42)), Ok(42));
        assert_eq!(
            super::caught_capture::<()>(|| Err("cancelled".into())),
            Err("cancelled".into())
        );
        assert_eq!(
            super::caught_capture::<()>(|| panic!("synthetic worker fault")),
            Err("The native capture worker could not complete.".into())
        );
    }
    use super::*;
    #[test]
    fn zoom_matches_source_fit_and_width() {
        assert_eq!(Zoom::Fit.scale((1200., 2400.), (600., 500.)), 500. / 2400.);
        assert_eq!(Zoom::FitWidth.scale((1200., 2400.), (600., 500.)), 0.5);
        assert_eq!(Zoom::Scale(20.).scale((1., 1.), (1., 1.)), 8.);
        assert_eq!(Zoom::Scale(f32::NAN).scale((1., 1.), (1., 1.)), 1.);
    }
    #[test]
    fn tool_order_matches_annotation_toolbar() {
        assert_eq!(
            TOOLS.map(|(_, label, _)| label),
            [
                "Select",
                "Pen",
                "Arrow",
                "Rectangle",
                "Highlight",
                "Text",
                "Crop",
                "Mask",
                "Eraser"
            ]
        );
    }

    #[test]
    fn mouse_release_preserves_final_point_and_bounds_stroke_size() {
        for tool in [
            AnnotationTool::Pen,
            AnnotationTool::Eraser,
            AnnotationTool::Arrow,
            AnnotationTool::Crop,
        ] {
            let mut g = Gesture {
                tool,
                points: vec![Point::new(1., 2.)],
                selected: None,
            };
            complete_gesture(&mut g, Point::new(30., 40.));
            assert_eq!(g.points.last(), Some(&Point::new(30., 40.)));
        }
        let mut g = Gesture {
            tool: AnnotationTool::Eraser,
            points: vec![Point::new(0., 0.); 20_000],
            selected: None,
        };
        complete_gesture(&mut g, Point::new(50., 50.));
        assert_eq!(g.points.len(), 20_000);
        assert_eq!(g.points.last(), Some(&Point::new(50., 50.)));
    }

    #[test]
    fn edited_screenshot_rejects_inflight_ocr_and_clears_all_copy_payloads() {
        let png = bellobox_core::qr::png("synthetic screenshot").unwrap();
        let mut session = ScreenshotEditSession::new(ScreenshotDocument::from_png(&png).unwrap());
        let mut jobs = SessionJobs::default();
        for edit in 0..3 {
            let revision = session.revision();
            let ticket = OcrTicket {
                job: jobs.begin(),
                revision,
            };
            let mut content = RevisionText {
                revision: Some(revision),
                text: "SYNTHETIC OLD OCR".into(),
            };
            assert!(ticket.accepts(&jobs, revision));
            assert!(content.current(revision).is_some());
            match edit {
                0 => {
                    session
                        .add_annotation(
                            AnnotationKind::Blur(Rect::new(0., 0., 20., 20.)),
                            AnnotationStyle::redaction(),
                        )
                        .unwrap();
                }
                1 => {
                    session.set_crop(Some(Rect::new(0., 0., 30., 30.))).unwrap();
                }
                _ => {
                    assert!(session.undo());
                }
            }
            // Even before cancellation, a changed document revision rejects the late response.
            assert!(!ticket.accepts(&jobs, session.revision()));
            assert!(content.current(session.revision()).is_none());
            jobs.cancel();
            content.invalidate();
            assert!(content.text.is_empty());
            assert!(content.current(revision).is_none());
            assert!(!ticket.accepts(&jobs, revision));
        }
    }
    #[test]
    fn preview_only_requests_tiles_intersecting_viewport() {
        let image = Arc::new(Image::empty());
        let first = PreviewTile {
            x: 0,
            y: 0,
            width: 1024,
            height: 1024,
            image: image.clone(),
        };
        let second = PreviewTile {
            x: 0,
            y: 1024,
            width: 1024,
            height: 1024,
            image,
        };
        assert!(tile_is_visible(
            &first,
            1.,
            (0., 0.),
            point(px(0.), px(0.)),
            (800., 700.)
        ));
        assert!(!tile_is_visible(
            &second,
            1.,
            (0., 0.),
            point(px(0.), px(0.)),
            (800., 700.)
        ));
        assert!(tile_is_visible(
            &second,
            1.,
            (0., 0.),
            point(px(0.), px(-1000.)),
            (800., 700.)
        ));
        assert!(!tile_is_visible(
            &first,
            1.,
            (0., 0.),
            point(px(0.), px(-1024.)),
            (800., 700.)
        ));
    }

    #[test]
    fn label_drag_threshold_is_in_view_points() {
        assert!(!label_drag_exceeds_threshold(
            Point::new(2., 3.),
            Point::new(3., 3.),
            1.
        ));
        assert!(label_drag_exceeds_threshold(
            Point::new(2., 3.),
            Point::new(3., 3.),
            2.
        ));
        assert!(!label_drag_exceeds_threshold(
            Point::new(2., 3.),
            Point::new(4., 3.),
            0.5
        ));
        assert!(!label_drag_exceeds_threshold(
            Point::new(0., 0.),
            Point::new(10., 10.),
            f32::NAN
        ));
    }
    #[test]
    fn transient_label_movement_preserves_undo_and_original_document() {
        let Ok(font) = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") else {
            return;
        };
        let png = bellobox_core::qr::png("synthetic").unwrap();
        let mut session = ScreenshotEditSession::new(ScreenshotDocument::from_png(&png).unwrap());
        session.set_text_font(&font).unwrap();
        let id = session
            .add_annotation(
                AnnotationKind::Text {
                    text: "LABEL".into(),
                    origin: Point::new(10., 20.),
                    max_width: 260.,
                },
                AnnotationStyle::default(),
            )
            .unwrap();
        let revision = session.revision();
        for x in [30., 40., 50.] {
            let drag = LabelDrag {
                id: Some(id),
                start: Point::new(10., 20.),
                origin: Point::new(10., 20.),
                current: Point::new(x, 40.),
                max_width: 260.,
                font_size: 18.,
                moved: true,
            };
            let preview = transient_label_snapshot(session.render_snapshot(), Some(drag)).unwrap();
            assert_eq!(preview.annotations()[0].kind.bounds().x, x);
            assert_eq!(session.document().annotations()[0].kind.bounds().x, 10.);
            assert_eq!(session.revision(), revision);
        }
        session.move_annotation(id, 40., 20.).unwrap();
        assert!(session.undo());
        assert_eq!(session.document().annotations()[0].kind.bounds().x, 10.);
        assert!(session.undo());
        assert!(session.document().annotations().is_empty());
        assert!(!session.undo());
    }
    #[test]
    fn label_preview_rejects_nontext_annotations() {
        let png = bellobox_core::qr::png("synthetic").unwrap();
        let mut session = ScreenshotEditSession::new(ScreenshotDocument::from_png(&png).unwrap());
        let id = session
            .add_annotation(
                AnnotationKind::Rectangle(Rect::new(1., 2., 20., 30.)),
                AnnotationStyle::default(),
            )
            .unwrap();
        let drag = LabelDrag {
            id: Some(id),
            start: Point::new(1., 2.),
            origin: Point::new(1., 2.),
            current: Point::new(30., 40.),
            max_width: 260.,
            font_size: 18.,
            moved: true,
        };
        assert!(transient_label_snapshot(session.render_snapshot(), Some(drag)).is_err());
    }
}
