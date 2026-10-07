//! Fresh reconstruction of the disabled main-display Area caller after workspace
//! loss. Source: CaptureOverlayController/ScreenCaptureService and the published
//! frozen Area model. This is not an identical recovery of an unpublished patch.
mod native;
mod selector;
#[cfg(test)]
mod tests;
use super::{
    CaptureChooser, NativeCaptureVisibility,
    area_transaction::{AreaTransaction, preflight},
};
use bello_platform::{macos_capture_overlay as host, native_capture as capture};
use bellobox_core::screenshot::{
    PreviewTile, Rect, ScreenshotDocument,
    area::{AreaDisplayGeometry, FrozenAreaSession},
};
use gpui::{AnyWindowHandle, App, Window};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// No production fixture/environment bypass. Native compilation/runtime review must approve
// production enablement separately, including callback and visibility behavior.
pub(super) const PRODUCTION_AREA_ENABLED: bool = false;
pub(super) const PRODUCTION_WINDOW_ENABLED: bool = false;

#[derive(Clone, Copy)]
enum CaptureIntent {
    Area,
    Window,
}
impl CaptureIntent {
    fn admitted<T>(self, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let (enabled, name) = match self {
            Self::Area => (PRODUCTION_AREA_ENABLED, "Area"),
            Self::Window => (PRODUCTION_WINDOW_ENABLED, "Window"),
        };
        if !enabled {
            return Err(format!(
                "{name} · main display only is awaiting native review."
            ));
        }
        work()
    }
}
type WindowPreparation =
    dyn FnOnce(Arc<AtomicBool>) -> Result<Arc<super::window_workflow::Source>, String> + Send;
enum Preparation {
    Area,
    Window(Box<WindowPreparation>),
}
const SNAPSHOT_DELAY: std::time::Duration = std::time::Duration::from_millis(60);

#[derive(Default)]
struct Coordinator {
    generation: u64,
    next_id: u64,
    active: Option<ActiveCapture>,
    cleanup_notice: Option<String>,
}
impl gpui::Global for Coordinator {}
#[derive(Clone, Copy, Default)]
enum HostSource {
    #[default]
    Native,
    #[cfg(any(debug_assertions, test))]
    SuppliedPixels {
        application_active: bool,
        topology_valid: bool,
    },
}
struct ActiveCapture<H = AnyWindowHandle, O = host::ApplicationDeactivationObserver> {
    source: HostSource,
    selection_locked: bool,
    transaction: AreaTransaction<H>,
    layout: host::MainDisplayOverlayLayout,
    // UI-owned !Send/!Sync native guard. Only the pointer-free signal is awaited.
    deactivation: Option<O>,
}
impl<H: Copy + Eq, O> ActiveCapture<H, O> {
    fn cancel(&mut self) -> Option<O> {
        self.transaction.cancel();
        self.deactivation.take()
    }
    fn lock_selection(&mut self) -> Option<O> {
        self.selection_locked = true;
        self.deactivation.take()
    }
    fn accepts_deactivation(
        &self,
        id: u64,
        generation: u64,
        current: u64,
        event: host::ApplicationDeactivationEvent,
    ) -> bool {
        // The notification has ALREADY cancelled the flag before waking us.
        // Do not require !cancelled/transaction.allows() here.
        event == host::ApplicationDeactivationEvent::ResignedActive
            && !self.selection_locked
            && self.deactivation.is_some()
            && self.transaction.id == id
            && self.transaction.generation == generation
            && generation == current
    }
}
struct PreparedArea {
    selection: selector::Selection,
    tiles: Vec<PreviewTile>,
}
fn geometry(layout: host::MainDisplayOverlayLayout) -> AreaDisplayGeometry {
    AreaDisplayGeometry {
        display_id: layout.display.id,
        cocoa_frame: Rect::new(
            layout.cocoa_frame.origin.x as f32,
            layout.cocoa_frame.origin.y as f32,
            layout.cocoa_frame.size.width as f32,
            layout.cocoa_frame.size.height as f32,
        ),
        pixel_size: (layout.display.pixels.width, layout.display.pixels.height),
        rotation_degrees: layout.rotation_degrees,
    }
}
fn validate_viewport(
    layout: host::MainDisplayOverlayLayout,
    viewport: capture::CaptureSize,
    scale: f64,
    display_id: Option<u32>,
) -> Result<(), String> {
    layout
        .validate_viewport(viewport)
        .map_err(|e| e.to_string())?;
    if !scale.is_finite()
        || (scale - layout.backing_scale).abs() > 0.01
        || display_id != Some(layout.display.id)
    {
        return Err(host::OverlayError::ViewportMismatch.to_string());
    }
    Ok(())
}
fn active_mut(id: u64, cx: &mut App) -> Option<&mut ActiveCapture> {
    cx.default_global::<Coordinator>()
        .active
        .as_mut()
        .filter(|run| run.transaction.id == id)
}
pub(super) fn take_notice(cx: &mut App) -> Option<String> {
    cx.default_global::<Coordinator>().cleanup_notice.take()
}
pub(super) fn navigation_changed(cx: &mut App) {
    let coordinator = cx.default_global::<Coordinator>();
    coordinator.generation = coordinator.generation.wrapping_add(1);
    let id = coordinator.active.as_ref().map(|run| run.transaction.id);
    if let Some(id) = id {
        cancel(id, cx);
    }
}
pub(super) fn requester_closed(requester: AnyWindowHandle, cx: &mut App) {
    let id = cx
        .try_global::<Coordinator>()
        .and_then(|state| state.active.as_ref())
        .filter(|run| run.transaction.requester == requester)
        .map(|run| run.transaction.id);
    if let Some(id) = id {
        cancel(id, cx);
    }
}

/// The click callback already borrows requester Window and root view. Never
/// reenter that logical handle: use the supplied window directly throughout hide
/// and synchronous failure restoration. Other windows are borrowed one at a time.
pub(super) fn begin(window: &mut Window, cx: &mut App) -> Result<(), String> {
    CaptureIntent::Area.admitted(|| begin_native(window, cx, |_| Ok(Preparation::Area)))
}
pub(super) fn begin_window(window: &mut Window, cx: &mut App) -> Result<(), String> {
    begin_window_with(window, cx, || {
        Arc::new(super::window_workflow::NativeBackend)
    })
}
fn begin_window_with(
    window: &mut Window,
    cx: &mut App,
    backend: impl FnOnce() -> Arc<dyn super::window_workflow::Backend>,
) -> Result<(), String> {
    // Check the independent Window gate before even constructing a backend or
    // reading displays, catalogs, permissions, clipboard or presentation state.
    CaptureIntent::Window.admitted(|| {
        with_window_capability(backend, |backend| {
            begin_native(window, cx, |layout| {
                Ok(Preparation::Window(Box::new(move |cancellation| {
                    super::window_workflow::Source::from_backend_with_request(
                        layout,
                        backend,
                        capture::WindowObservationRequest::new(
                            capture::CaptureCancellation::from_flag(cancellation),
                            std::time::Instant::now() + capture::MAX_CAPTURE_TIMEOUT,
                        ),
                    )
                })))
            })
        })
    })
}
fn with_window_capability<T>(
    backend: impl FnOnce() -> Arc<dyn super::window_workflow::Backend>,
    begin: impl FnOnce(Arc<dyn super::window_workflow::Backend>) -> Result<T, String>,
) -> Result<T, String> {
    let backend = backend();
    backend.check_available()?;
    begin(backend)
}
fn begin_native(
    window: &mut Window,
    cx: &mut App,
    prepare: impl FnOnce(host::MainDisplayOverlayLayout) -> Result<Preparation, String>,
) -> Result<(), String> {
    if cx.default_global::<NativeCaptureVisibility>().busy {
        return Err("Another capture is still finishing.".into());
    }
    let layout = host::main_display_overlay_layout().map_err(|e| e.to_string())?;
    layout.validate().map_err(|e| e.to_string())?;
    let preparation = prepare(layout)?;
    let requester = window.window_handle();
    let context = native::context(window)?;
    if !context.requester_visible || !context.allows_capture_continuation(0, 0) {
        return Err(host::OverlayError::NavigationChanged.to_string());
    }
    let handles = cx.windows();
    let prior_visible = preflight(&handles, |handle| {
        if handle == requester {
            return native::context(window).map(|c| c.requester_visible);
        }
        handle
            .update(cx, |_, owned, _| {
                native::context(owned).map(|c| c.requester_visible)
            })
            .map_err(|e| e.to_string())?
    })?;
    if !prior_visible.contains(&requester) {
        return Err(host::OverlayError::NavigationChanged.to_string());
    }
    let coordinator = cx.default_global::<Coordinator>();
    coordinator.next_id = coordinator.next_id.wrapping_add(1);
    let id = coordinator.next_id;
    // All prior-visible handles are recorded before the first native mutation.
    coordinator.active = Some(ActiveCapture {
        source: HostSource::Native,
        selection_locked: false,
        transaction: AreaTransaction::new(
            id,
            coordinator.generation,
            requester,
            prior_visible.clone(),
        ),
        layout,
        deactivation: None,
    });
    cx.default_global::<NativeCaptureVisibility>().busy = true;
    for handle in prior_visible {
        let result = if handle == requester {
            native::hide(window)
        } else {
            handle
                .update(cx, |_, owned, _| native::hide(owned))
                .map_err(|e| e.to_string())
                .and_then(|r| r)
        };
        if let Err(error) = result {
            // orderOut may have succeeded before native postcondition validation.
            mark_cancelled(id, cx);
            let restored = restore_recorded(id, Some(window), cx);
            finish_if_ready(id, cx);
            return Err(restored.err().unwrap_or(error));
        }
    }
    let cancellation = {
        let run = active_mut(id, cx).expect("new Area transaction");
        run.transaction.start_worker();
        run.transaction.cancellation.clone()
    };
    let task = cx.background_executor().spawn(async move {
        super::caught_capture(|| {
            // Window's subsequent catalog shares native admission with Display.
            // Its frozen dependency must physically drain before catalog entry.
            let freeze = if matches!(&preparation, Preparation::Window(_)) {
                capture::capture_window_freeze
            } else {
                capture::capture
            };
            prepare_capture_with(layout, cancellation, preparation, freeze)
        })
    });
    // App-owned cleanup is independent of requester/window/entity liveness.
    cx.spawn(async move |cx| {
        let result = task.await;
        let _ = cx.update(|cx| freeze_completed(id, result, cx));
    })
    .detach();
    Ok(())
}
#[cfg(any(debug_assertions, test))]
fn prepare_area_with(
    layout: host::MainDisplayOverlayLayout,
    cancellation: Arc<AtomicBool>,
    capture: impl FnOnce(
        capture::CaptureRequest,
        capture::CaptureCancellation,
    ) -> capture::CaptureResult<capture::NativeCaptureSnapshot>,
) -> Result<PreparedArea, String> {
    prepare_capture_with(layout, cancellation, Preparation::Area, capture)
}
fn prepare_capture_with(
    layout: host::MainDisplayOverlayLayout,
    cancellation: Arc<AtomicBool>,
    preparation: Preparation,
    capture: impl FnOnce(
        capture::CaptureRequest,
        capture::CaptureCancellation,
    ) -> capture::CaptureResult<capture::NativeCaptureSnapshot>,
) -> Result<PreparedArea, String> {
    let active = || {
        if cancellation.load(Ordering::Acquire) {
            Err("Area capture was cancelled.".to_owned())
        } else {
            Ok(())
        }
    };
    active()?;
    std::thread::sleep(SNAPSHOT_DELAY);
    active()?;
    // Exactly one explicit-display capture, before any selector is created.
    // Only immutable UI-thread metadata crosses into this worker.
    let snapshot = capture(
        capture::CaptureRequest::full_display(layout.display),
        capture::CaptureCancellation::from_flag(cancellation.clone()),
    )
    .map_err(|e| e.to_string())?;
    active()?;
    if snapshot.diagnostics.resolved_display_id != layout.display.id
        || snapshot.diagnostics.output_size != layout.display.pixels
    {
        return Err(host::OverlayError::DisplayChanged.to_string());
    }
    let document = ScreenshotDocument::from_png(&snapshot.png)?;
    drop(snapshot);
    active()?;
    let display = geometry(layout);
    if document.dimensions() != display.pixel_size {
        return Err("Frozen display dimensions changed.".into());
    }
    let tiles = document.render_preview_tiles()?;
    active()?;
    let selection = match preparation {
        Preparation::Area => {
            let mut area = FrozenAreaSession::new(display).map_err(|e| e.to_string())?;
            let token = area
                .freeze_token()
                .ok_or("Area freeze was already consumed.")?;
            if !area
                .accept_frozen(token, document, display)
                .map_err(|e| e.to_string())?
            {
                return Err("Area freeze was superseded.".into());
            }
            selector::Selection::Area(area)
        }
        Preparation::Window(source) => {
            use bellobox_core::screenshot::window::FrozenWindowSession;
            // Source catalogs the windows only after the full-display freeze,
            // before an overlay exists. The exact native catalog stays gated.
            let source = source(cancellation.clone())?;
            let mut window =
                FrozenWindowSession::new(display, &source.candidates()?, source.own_pid())
                    .map_err(|error| error.to_string())?;
            let token = window
                .freeze_token()
                .ok_or("Window freeze was already consumed.")?;
            if !window
                .accept_frozen(token, document, display)
                .map_err(|e| e.to_string())?
            {
                return Err("Window freeze was superseded.".into());
            }
            selector::Selection::Window(window, source)
        }
    };
    active()?;
    Ok(PreparedArea { selection, tiles })
}

pub(super) fn current(id: u64, owner: &Window, cx: &App) -> bool {
    let Some(state) = cx.try_global::<Coordinator>() else {
        return false;
    };
    let Some(run) = state.active.as_ref().filter(|run| run.transaction.id == id) else {
        return false;
    };
    let (context, topology) = match run.source {
        HostSource::Native => {
            let context = if run.selection_locked {
                None
            } else {
                native::context(owner).ok()
            };
            (
                context,
                host::main_display_overlay_layout()
                    .and_then(|now| run.layout.revalidate(now))
                    .is_ok(),
            )
        }
        #[cfg(any(debug_assertions, test))]
        HostSource::SuppliedPixels {
            application_active,
            topology_valid,
        } => (
            Some(host::OwnedWindowContext {
                application_active,
                application_hidden: false,
                requester_visible: true,
                key_window: host::OwnedKeyWindow::Requester,
            }),
            topology_valid,
        ),
    };
    let live = cx.windows().contains(&run.transaction.requester);
    if run.selection_locked {
        // Lock is the source lifecycle boundary. An inactive app must not retire
        // its editor or force activation when asynchronous font preparation ends.
        !run.transaction.cancelled()
            && live
            && topology
            && run.transaction.generation == state.generation
    } else {
        context.is_some_and(|context| {
            run.transaction
                .allows(state.generation, live, context, topology)
        })
    }
}
fn current_requester(id: u64, cx: &mut App) -> bool {
    let Some(requester) = active_mut(id, cx).map(|run| run.transaction.requester) else {
        return false;
    };
    requester
        .update(cx, |_, window, cx| current(id, window, cx))
        .unwrap_or(false)
}
fn mark_cancelled(id: u64, cx: &mut App) {
    let guard = active_mut(id, cx).and_then(ActiveCapture::cancel);
    unregister(guard);
}
fn lock_selection(id: u64, cx: &mut App) {
    let guard = active_mut(id, cx).and_then(ActiveCapture::lock_selection);
    unregister(guard);
}
#[cfg_attr(
    not(all(target_os = "macos", target_arch = "aarch64")),
    allow(clippy::drop_non_drop)
)]
fn unregister(guard: Option<host::ApplicationDeactivationObserver>) {
    // Explicit native RAII boundary, after mutable global borrow has ended. The
    // non-native test stub has no native Drop implementation.
    drop(guard);
}
fn observe_deactivation(id: u64, cx: &mut App) -> Result<(), String> {
    if is_supplied(id, cx) {
        return Ok(());
    }
    let (generation, cancellation) = active_mut(id, cx)
        .filter(|run| !run.transaction.cancelled() && run.deactivation.is_none())
        .map(|run| {
            (
                run.transaction.generation,
                run.transaction.cancellation.clone(),
            )
        })
        .ok_or_else(|| host::OverlayError::NavigationChanged.to_string())?;
    let (guard, signal) = host::observe_application_deactivation(
        capture::CaptureCancellation::from_flag(cancellation),
    )
    .map_err(|e| e.to_string())?;
    let Some(run) = active_mut(id, cx).filter(|run| run.transaction.generation == generation)
    else {
        unregister(Some(guard));
        return Err(host::OverlayError::NavigationChanged.to_string());
    };
    run.deactivation = Some(guard);
    cx.spawn(async move |cx| {
        let event = signal.await;
        let _ = cx.update(|cx| {
            let matches = cx.try_global::<Coordinator>().is_some_and(|state| {
                state.active.as_ref().is_some_and(|run| {
                    run.accepts_deactivation(id, generation, state.generation, event)
                })
            });
            if matches {
                cancel(id, cx);
            }
        });
    })
    .detach();
    Ok(())
}
fn freeze_completed(id: u64, result: Result<PreparedArea, String>, cx: &mut App) {
    let Some(run) = active_mut(id, cx) else {
        discard_untracked(result, cx);
        return;
    };
    run.transaction.finish_worker();
    // Read continuity BEFORE restoration could obscure a newer navigation.
    let valid = current_requester(id, cx);
    let restoration = restore_recorded(id, None, cx);
    if !valid || restoration.is_err() {
        mark_cancelled(id, cx);
        discard(id, result, cx);
        return;
    }
    match result {
        Ok(prepared) => {
            if !active_mut(id, cx).is_some_and(|run| run.transaction.accept_freeze())
                || !current_requester(id, cx)
            {
                mark_cancelled(id, cx);
                discard(id, prepared, cx);
                return;
            }
            // Source observes application deactivation after freezing. Register
            // before selector creation; a preserved key-window flag is insufficient.
            if let Err(error) = observe_deactivation(id, cx) {
                mark_cancelled(id, cx);
                report(id, error, cx);
                discard(id, prepared, cx);
                return;
            }
            if !current_requester(id, cx) {
                mark_cancelled(id, cx);
                discard(id, prepared, cx);
                return;
            }
            let layout = active_mut(id, cx)
                .expect("accepted Area transaction")
                .layout;
            if let Err(error) = selector::open(id, layout, prepared, cx) {
                mark_cancelled(id, cx);
                report(id, error, cx);
                finish_if_ready(id, cx);
            }
        }
        Err(error) => {
            mark_cancelled(id, cx);
            report(id, error, cx);
            finish_if_ready(id, cx);
        }
    }
}
fn restore_recorded(
    id: u64,
    mut borrowed: Option<&mut Window>,
    cx: &mut App,
) -> Result<(), String> {
    let live = cx.windows();
    let handles = match active_mut(id, cx) {
        Some(run) => {
            let handles = run.transaction.restore_live(&live);
            run.transaction.restore.clear();
            handles
        }
        None => return Ok(()),
    };
    let mut first_error = None;
    for handle in handles {
        let result = if let Some(window) = borrowed
            .as_deref_mut()
            .filter(|w| w.window_handle() == handle)
        {
            native::restore(window)
        } else {
            handle
                .update(cx, |_, window, _| native::restore(window))
                .map_err(|e| e.to_string())
                .and_then(|r| r)
        };
        if let Err(error) = result {
            first_error.get_or_insert(error);
        }
    }
    if let Some(error) = first_error {
        let notice = format!(
            "Area capture could not safely restore a prior window: {error} Reopen the desired tool if needed."
        );
        cx.default_global::<Coordinator>().cleanup_notice = Some(notice.clone());
        if borrowed.is_none() {
            report(id, notice, cx);
        }
        // One safe attempt only. No forced/front-order/activation fallback.
        Err(error)
    } else {
        Ok(())
    }
}
fn report(id: u64, status: String, cx: &mut App) {
    let requester = active_mut(id, cx).map(|run| run.transaction.requester);
    if let Some(handle) = requester.and_then(|h| h.downcast::<CaptureChooser>()) {
        let _ = handle.update(cx, |chooser, _, cx| {
            chooser.status = status;
            cx.notify();
        });
    }
}
fn cleanup_notice(id: u64, notice: String, cx: &mut App) {
    cx.default_global::<Coordinator>().cleanup_notice = Some(notice.clone());
    report(id, notice, cx);
}
pub(super) fn cancel(id: u64, cx: &mut App) {
    mark_cancelled(id, cx);
    // Avoid reentering the requester or selector from its current callback.
    cx.defer(move |cx| {
        let selector = active_mut(id, cx).and_then(|run| run.transaction.selector);
        if let Some(handle) = selector.and_then(|h| h.downcast::<selector::MainAreaSelector>()) {
            let _ = handle.update(cx, |view, window, cx| view.retire(window, cx));
        }
        // Cancellation renders pending pixels unusable: restore promptly while
        // the heavy worker drains, but keep global busy via the worker count.
        let _ = restore_recorded(id, None, cx);
        finish_if_ready(id, cx);
    });
}
pub(super) fn discard<T: Send + 'static>(id: u64, value: T, cx: &mut App) {
    if let Some(run) = active_mut(id, cx) {
        run.transaction.start_worker();
    }
    let task = cx.background_executor().spawn(async move {
        drop(value);
    });
    cx.spawn(async move |cx| {
        task.await;
        let _ = cx.update(|cx| {
            if let Some(run) = active_mut(id, cx) {
                run.transaction.finish_worker();
            }
            let _ = restore_recorded(id, None, cx);
            finish_if_ready(id, cx);
        });
    })
    .detach();
}
fn discard_untracked<T: Send + 'static>(value: T, cx: &App) {
    cx.background_executor()
        .spawn(async move {
            drop(value);
        })
        .detach();
}
fn finish_if_ready(id: u64, cx: &mut App) {
    if !active_mut(id, cx).is_some_and(|run| run.transaction.can_release()) {
        return;
    }
    let mut run = cx
        .default_global::<Coordinator>()
        .active
        .take()
        .expect("completed Area transaction");
    unregister(run.deactivation.take());
    cx.default_global::<NativeCaptureVisibility>().busy = false;
    let requester = run.transaction.requester;
    cx.defer(move |cx| {
        if let Some(handle) = requester.downcast::<CaptureChooser>() {
            let _ = handle.update(cx, |chooser, _, cx| {
                chooser.busy = false;
                if matches!(
                    chooser.status.as_str(),
                    "Freezing the main display…" | "Selecting from supplied synthetic pixels…"
                ) {
                    chooser.status.clear();
                }
                cx.notify();
            });
        }
    });
}

fn is_supplied(id: u64, cx: &App) -> bool {
    #[cfg(any(debug_assertions, test))]
    {
        cx.try_global::<Coordinator>()
            .and_then(|state| state.active.as_ref())
            .is_some_and(|run| {
                run.transaction.id == id && matches!(run.source, HostSource::SuppliedPixels { .. })
            })
    }
    #[cfg(not(any(debug_assertions, test)))]
    {
        let _ = (id, cx);
        false
    }
}
/// Explicit supplied pixels enter the same freeze completion, selector, editor
/// and retirement path. This never calls begin(), changes its gate, or invokes
/// native capture, permissions, owned-window hiding or native observations.
#[cfg(debug_assertions)]
pub(super) fn begin_fixture(window: &mut Window, cx: &mut App) -> Result<(), String> {
    let display = cx
        .displays()
        .into_iter()
        .next()
        .ok_or("No fixture display.")?;
    let layout = host::MainDisplayOverlayLayout {
        display: capture::CaptureDisplay {
            // X11 may expose GPUI display ID 0, which is not a valid CG ID.
            // This supplied-pixel identity never reaches a native API.
            id: u32::from(display.id()).max(1),
            bounds: capture::CaptureRect::new(0., 0., 1000., 640.),
            pixels: capture::CapturePixelSize {
                width: 2000,
                height: 1280,
            },
        },
        cocoa_frame: capture::CaptureRect::new(0., 0., 1000., 640.),
        rotation_degrees: 0,
        backing_scale: 2.,
    };
    begin_supplied(window.window_handle(), layout, cx, move |request, _| {
        let png = bellobox_core::screenshot::scroll::synthetic_page_frame(
            0,
            request.output_size.width,
            request.output_size.height,
            2500,
        )
        .map_err(|_| capture::CaptureError::InvalidImage)?
        .png()
        .map_err(|_| capture::CaptureError::InvalidImage)?;
        Ok(capture::NativeCaptureSnapshot {
            png,
            diagnostics: capture::CaptureDiagnostics {
                requested_display_id: request.display.id,
                resolved_display_id: request.display.id,
                resolution_path: capture::DisplayResolutionPath::InitialId,
                output_size: request.output_size,
                region: None,
                includes_cursor: false,
                backend: "supplied synthetic pixels",
            },
        })
    })
}
#[cfg(any(debug_assertions, test))]
fn begin_supplied(
    requester: AnyWindowHandle,
    layout: host::MainDisplayOverlayLayout,
    cx: &mut App,
    pixels: impl FnOnce(
        capture::CaptureRequest,
        capture::CaptureCancellation,
    ) -> capture::CaptureResult<capture::NativeCaptureSnapshot>
    + Send
    + 'static,
) -> Result<(), String> {
    begin_supplied_prepared(requester, layout, cx, move |cancellation| {
        prepare_area_with(layout, cancellation, pixels)
    })
}
#[cfg(any(debug_assertions, test))]
fn begin_supplied_prepared(
    requester: AnyWindowHandle,
    layout: host::MainDisplayOverlayLayout,
    cx: &mut App,
    prepare: impl FnOnce(Arc<AtomicBool>) -> Result<PreparedArea, String> + Send + 'static,
) -> Result<(), String> {
    layout.validate().map_err(|e| e.to_string())?;
    if cx.default_global::<NativeCaptureVisibility>().busy {
        return Err("Another capture is still finishing.".into());
    }
    let state = cx.default_global::<Coordinator>();
    state.next_id = state.next_id.wrapping_add(1);
    let id = state.next_id;
    let mut transaction = AreaTransaction::new(id, state.generation, requester, vec![]);
    transaction.start_worker();
    let cancellation = transaction.cancellation.clone();
    state.active = Some(ActiveCapture {
        transaction,
        layout,
        deactivation: None,
        selection_locked: false,
        source: HostSource::SuppliedPixels {
            application_active: true,
            topology_valid: true,
        },
    });
    cx.default_global::<NativeCaptureVisibility>().busy = true;
    let task = cx
        .background_executor()
        .spawn(async move { super::caught_capture(|| prepare(cancellation)) });
    cx.spawn(async move |cx| {
        let result = task.await;
        let _ = cx.update(|cx| freeze_completed(id, result, cx));
    })
    .detach();
    Ok(())
}

/// Every inline preview/OCR/export future is counted through physical completion,
/// independently of the selector's entity lifetime.
pub(super) fn begin_editor_work(id: u64, cx: &mut App) -> Option<u64> {
    let run = active_mut(id, cx).filter(|run| !run.transaction.cancelled())?;
    run.transaction.start_worker();
    Some(id)
}
pub(super) fn finish_editor_work(id: Option<u64>, cx: &mut App) {
    if let Some(id) = id {
        if let Some(run) = active_mut(id, cx) {
            run.transaction.finish_worker();
        }
        finish_if_ready(id, cx);
    }
}
pub(super) fn editor_current(id: u64, cx: &mut App) -> bool {
    current_requester(id, cx)
}

pub(super) fn editor_cancellation(id: u64, cx: &mut App) -> Option<Arc<AtomicBool>> {
    active_mut(id, cx).map(|run| run.transaction.cancellation.clone())
}

/// This supplied-record Window route uses the same coordinator and owned overlay.
/// It never invokes native catalog enumeration or the disabled capture_window API.
#[cfg(debug_assertions)]
pub(super) fn begin_window_fixture(window: &mut Window, cx: &mut App) -> Result<(), String> {
    let layout = super::window_workflow::fixture_layout();
    begin_supplied_prepared(window.window_handle(), layout, cx, move |cancellation| {
        prepare_window_supplied(super::window_workflow::fixture_for_ui()?, cancellation)
    })
}
#[cfg(test)]
fn begin_window_supplied(
    requester: AnyWindowHandle,
    fixture: super::window_workflow::Fixture,
    cx: &mut App,
) -> Result<(), String> {
    begin_supplied_prepared(requester, fixture.layout, cx, move |cancellation| {
        prepare_window_supplied(fixture, cancellation)
    })
}
#[cfg(any(debug_assertions, test))]
fn prepare_window_supplied(
    fixture: super::window_workflow::Fixture,
    cancellation: Arc<AtomicBool>,
) -> Result<PreparedArea, String> {
    prepare_capture_with(
        fixture.layout,
        cancellation,
        Preparation::Window(Box::new(move |_| Ok(fixture.source))),
        move |request, _| {
            Ok(capture::NativeCaptureSnapshot {
                png: fixture
                    .document
                    .render_png()
                    .map_err(|_| capture::CaptureError::InvalidImage)?,
                diagnostics: capture::CaptureDiagnostics {
                    requested_display_id: request.display.id,
                    resolved_display_id: request.display.id,
                    resolution_path: capture::DisplayResolutionPath::InitialId,
                    output_size: request.output_size,
                    region: None,
                    includes_cursor: false,
                    backend: "supplied synthetic pixels",
                },
            })
        },
    )
}
