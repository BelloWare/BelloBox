//! Main-queue catalog operations. Only owned values leave this invocation.
use super::*;
pub(super) type CatalogJob = Job<WindowCatalogSnapshot, WindowCaptureError>;

pub(in crate::native_capture) fn identity_and_layers(
) -> WindowCaptureResult<(i32, WindowCatalogLayers)> {
    let pid = i32::try_from(std::process::id()).map_err(|_| WindowPolicyError::InvalidMetadata)?;
    // SDK CGWindowLevelKey discriminants, not presumed numeric level values.
    // These match objc2-core-graphics 0.3.2 generated CGWindowLevel.rs.
    unsafe {
        Ok((
            pid,
            WindowCatalogLayers {
                normal: i64::from(CGWindowLevelForKey(4)),
                floating: i64::from(CGWindowLevelForKey(5)),
                modal_panel: i64::from(CGWindowLevelForKey(10)),
                main_menu: i64::from(CGWindowLevelForKey(8)),
                status: i64::from(CGWindowLevelForKey(9)),
                popup_menu: i64::from(CGWindowLevelForKey(11)),
                screen_saver: i64::from(CGWindowLevelForKey(13)),
            },
        ))
    }
}

pub(in crate::native_capture) fn observe(
    request: WindowObservationRequest,
) -> WindowCaptureResult<WindowCatalogSnapshot> {
    // Reject before dispatch-and-wait, avoiding main-queue self-deadlock.
    require_worker_thread()?;
    request.check()?;
    let lease = InflightGuard::acquire(&CAPTURE_IN_FLIGHT)?;
    let drain = lease.drain();
    let job = CatalogJob::with_deadline(request.cancellation(), request.deadline());
    request.check()?;
    unsafe {
        enqueue_context(MainContext::Catalog {
            request: request.clone(),
            job: job.clone(),
            _lease: lease,
        });
    }
    let result = job.wait_drained(drain);
    request.check()?;
    result
}

/// A null query is unavailable (None), distinct from an empty array. Every
/// dictionary keeps its position, including a target row with no other fields.
unsafe fn raw_rows(
    array: Id,
    check: impl Fn() -> WindowCaptureResult<()>,
) -> WindowCaptureResult<Option<Vec<RawWindowOcclusionRow>>> {
    check()?;
    if array.is_null() {
        return Ok(None);
    }
    if CFGetTypeID(array) != CFArrayGetTypeID() {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let count = CFArrayGetCount(array);
    if count < 0 || count as usize > MAX_WINDOW_CANDIDATES {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let mut result = Vec::with_capacity(count as usize);
    for index in 0..count {
        check()?;
        let row = CFArrayGetValueAtIndex(array, index);
        if row.is_null() || CFGetTypeID(row) != CFDictionaryGetTypeID() {
            return Err(WindowPolicyError::InvalidMetadata.into());
        }
        let window_id = integer(row, kCGWindowNumber)
            .ok()
            .and_then(|value| u32::try_from(value).ok());
        let owner_process_id = integer(row, kCGWindowOwnerPID)
            .ok()
            .and_then(|value| i32::try_from(value).ok());
        let layer = integer(row, kCGWindowLayer).ok();
        let alpha = number(row, kCGWindowAlpha).ok().and_then(|number| {
            let mut value = 0f64;
            (CFNumberGetValue(number, 6, (&mut value as *mut f64).cast()) != 0).then_some(value)
        });
        let bounds = CFDictionaryGetValue(row, kCGWindowBounds);
        let frame = if !bounds.is_null() && CFGetTypeID(bounds) == CFDictionaryGetTypeID() {
            let mut value = CaptureRect::new(0., 0., 0., 0.);
            CGRectMakeWithDictionaryRepresentation(bounds, &mut value).then_some(value)
        } else {
            None
        };
        result.push(RawWindowOcclusionRow {
            window_id,
            owner_process_id,
            layer,
            alpha,
            frame,
        });
    }
    check()?;
    Ok(Some(result))
}

pub(super) unsafe fn collect(
    request: &WindowObservationRequest,
) -> WindowCaptureResult<WindowCatalogSnapshot> {
    require_main_thread()?;
    request.check()?;
    let _pool = pool()?;
    let topology = current_topology(|| request.check())?;
    request.check()?;
    let strict_catalog = OwnedCf::new(CGWindowListCopyWindowInfo(1 | 16, 0))?;
    request.check()?;
    let strict = observations_checked(strict_catalog.0, || request.check())?;
    let observations = strict
        .into_iter()
        .map(|row| OwnedWindowObservation {
            window_id: row.identity.window_id,
            owner_process_id: row.identity.owner_process_id,
            owner_bundle_id: None,
            frame: row.frame.0,
            layer: row.layer,
            alpha: row.alpha,
            on_screen: row.on_screen,
        })
        .collect();
    drop(strict_catalog);
    request.check()?;
    let raw_catalog = CGWindowListCopyWindowInfo(1 | 16, 0);
    // A nullable OwnedCf cannot be constructed. Keep ownership explicit on every
    // parser error, cancellation and post-call timeout path.
    let raw_catalog = (!raw_catalog.is_null()).then(|| OwnedCf(raw_catalog));
    request.check()?;
    let occlusion_rows = raw_rows(
        raw_catalog
            .as_ref()
            .map_or(ptr::null_mut(), |owner| owner.0),
        || request.check(),
    )?;
    drop(raw_catalog);
    request.check()?;
    let final_topology = current_topology(|| request.check())?;
    request.check()?;
    if topology.main_display_id != final_topology.main_display_id
        || topology.displays.len() != final_topology.displays.len()
        || topology
            .displays
            .iter()
            .any(|d| !final_topology.displays.contains(d))
    {
        return Err(WindowPolicyError::TopologyChanged.into());
    }
    Ok(WindowCatalogSnapshot {
        observations,
        main_display_id: topology.main_display_id,
        displays: topology.displays,
        occlusion_rows,
    })
}

#[cfg(test)]
mod tests;
