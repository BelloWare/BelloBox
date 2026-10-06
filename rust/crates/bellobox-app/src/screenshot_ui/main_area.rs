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

// No fixture/environment bypass. Native compilation/runtime review must approve
// production enablement separately, including callback and visibility behavior.
pub(super) const PRODUCTION_AREA_ENABLED: bool = false;
const SNAPSHOT_DELAY: std::time::Duration = std::time::Duration::from_millis(60);

#[derive(Default)]
struct Coordinator {
    generation: u64,
    next_id: u64,
    active: Option<ActiveCapture>,
    cleanup_notice: Option<String>,
}
impl gpui::Global for Coordinator {}
struct ActiveCapture<H = AnyWindowHandle, O = host::ApplicationDeactivationObserver> {
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
    fn finish(&mut self) -> Option<O> {
        self.transaction.finish();
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
            && self.deactivation.is_some()
            && self.transaction.id == id
            && self.transaction.generation == generation
            && generation == current
    }
}
struct PreparedArea {
    area: FrozenAreaSession,
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
    if !PRODUCTION_AREA_ENABLED {
        return Err("Area · main display only is awaiting native review.".into());
    }
    if cx.default_global::<NativeCaptureVisibility>().busy {
        return Err("Another capture is still finishing.".into());
    }
    let layout = host::main_display_overlay_layout().map_err(|e| e.to_string())?;
    layout.validate().map_err(|e| e.to_string())?;
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
        super::caught_capture(|| prepare_area_with(layout, cancellation, capture::capture))
    });
    // App-owned cleanup is independent of requester/window/entity liveness.
    cx.spawn(async move |cx| {
        let result = task.await;
        let _ = cx.update(|cx| freeze_completed(id, result, cx));
    })
    .detach();
    Ok(())
}
fn prepare_area_with(
    layout: host::MainDisplayOverlayLayout,
    cancellation: Arc<AtomicBool>,
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
    let mut area = FrozenAreaSession::new(display).map_err(|e| e.to_string())?;
    let token = area
        .freeze_token()
        .ok_or("Area freeze was already consumed.")?;
    let tiles = document.render_preview_tiles()?;
    active()?;
    if !area
        .accept_frozen(token, document, display)
        .map_err(|e| e.to_string())?
    {
        return Err("Area freeze was superseded.".into());
    }
    active()?;
    Ok(PreparedArea { area, tiles })
}
fn current(id: u64, owner: &Window, cx: &App) -> bool {
    let Some(state) = cx.try_global::<Coordinator>() else {
        return false;
    };
    let Some(run) = state.active.as_ref().filter(|run| run.transaction.id == id) else {
        return false;
    };
    let Ok(context) = native::context(owner) else {
        return false;
    };
    let topology = host::main_display_overlay_layout()
        .and_then(|now| run.layout.revalidate(now))
        .is_ok();
    run.transaction.allows(
        state.generation,
        cx.windows().contains(&run.transaction.requester),
        context,
        topology,
    )
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
fn mark_finished(id: u64, cx: &mut App) {
    let guard = active_mut(id, cx).and_then(ActiveCapture::finish);
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
fn cancel(id: u64, cx: &mut App) {
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
fn discard<T: Send + 'static>(id: u64, value: T, cx: &mut App) {
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
                cx.notify();
            });
        }
    });
}
