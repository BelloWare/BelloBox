//! DEBUG-only Window refresh host. Generated independent pixels flow through the
//! real editor/preview/OCR path; no native capture API or permission is reachable.
use super::ScreenshotEditor;
use crate::session::{JobToken, SessionJobs};
use bellobox_core::screenshot::{
    window::{FrozenWindowCommit, synthetic_independent_window},
    window_refresh::{
        OcclusionLayers, OcclusionRow, PreparedWindowRefresh, WindowRefreshContext,
        WindowRefreshDecision, WindowRefreshPlan, WindowRefreshSource, decide_window_refresh,
    },
};
use gpui::Context;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct Request {
    pub context: WindowRefreshContext,
    pub decision: WindowRefreshDecision,
    pub boundary: Arc<AtomicBool>,
}
impl Request {
    pub fn fixture(commit: &FrozenWindowCommit, boundary: Arc<AtomicBool>) -> Result<Self, String> {
        let candidate = commit.candidate();
        // This is an explicitly synthetic occluder catalog, separate from selector
        // eligibility. A native caller must supply actual CG levels/full topology.
        let rows = [
            OcclusionRow {
                window_id: Some(1),
                owner_process_id: Some(101),
                layer: Some(0),
                alpha: Some(1.),
                frame: Some(super::Rect::new(300., 80., 340., 270.)),
            },
            OcclusionRow {
                window_id: Some(2),
                owner_process_id: Some(102),
                layer: Some(0),
                alpha: Some(1.),
                frame: Some(super::Rect::new(100., 170., 420., 230.)),
            },
        ];
        let decision = decide_window_refresh(
            WindowRefreshSource {
                independent_window: true,
                scrolling_active: false,
                cut_from_frozen: true,
                window_id: candidate.window_id,
                frame: Some(candidate.frame_local_points),
            },
            Some(&rows),
            999,
            OcclusionLayers {
                normal: 0,
                floating: 3,
                modal_panel: 8,
                main_menu: 24,
                status: 25,
                popup_menu: 101,
                screen_saver: 1000,
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            context: WindowRefreshContext {
                selection: commit.token(),
                window_id: candidate.window_id,
                display: commit.geometry(),
            },
            decision,
            boundary,
        })
    }
}

pub(super) struct Host {
    context: WindowRefreshContext,
    decision: WindowRefreshDecision,
    boundary: Arc<AtomicBool>,
    jobs: SessionJobs,
}
impl ScreenshotEditor {
    pub(super) fn start_window_refresh(&mut self, request: Request, cx: &mut Context<Self>) {
        // A newer request retires the old one even if the new request is already
        // cancelled or fails validation before a worker is created.
        self.cancel_window_refresh();
        let mut host = Host {
            context: request.context,
            decision: request.decision,
            boundary: request.boundary,
            jobs: SessionJobs::default(),
        };
        let token = host.jobs.begin();
        let cancellation = host.jobs.cancellation();
        if host.boundary.load(Ordering::Acquire) {
            return;
        }
        let Ok(plan) = WindowRefreshPlan::new(
            &self.session,
            host.context,
            host.decision,
            cancellation.clone(),
        ) else {
            return;
        };
        let boundary = host.boundary.clone();
        let window_id = host.context.window_id;
        self.window_refresh = Some(host); // Replacing a prior host cancels its work.
        let task = cx.background_executor().spawn(async move {
            if boundary.load(Ordering::Acquire) || cancellation.load(Ordering::Acquire) {
                return None;
            }
            let independent = synthetic_independent_window(window_id, &cancellation).ok()?;
            if boundary.load(Ordering::Acquire) {
                return None;
            }
            prepare_refresh(plan, independent, cancellation)
                .ok()
                .flatten()
        });
        cx.spawn(async move |this, cx| {
            let prepared = task.await;
            let _ = this.update(cx, |this, cx| {
                this.accept_window_refresh(token, prepared, cx);
            });
        })
        .detach();
    }

    fn accept_window_refresh(
        &mut self,
        token: JobToken,
        prepared: Option<PreparedWindowRefresh>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(host) = self.window_refresh.as_mut() else {
            return false;
        };
        if !host.jobs.accepts(token) {
            return false;
        }
        // One-shot publication, including failures and clean-state rejection. Do
        // not retry after the user edits, cancels or exports the frozen capture.
        // Keep the local job flag alive until apply has checked it below.
        let idle = self.gesture.is_none()
            && self.text_origin.is_none()
            && self.label_drag.is_none()
            && !self.export_busy
            && !self.show_discard;
        let accepted = idle
            && prepared.is_some_and(|prepared| {
                prepared
                    .apply(&mut self.session, host.context, &host.boundary)
                    .unwrap_or(false)
            });
        let decision = host.decision;
        host.jobs.cancel();
        if accepted {
            self.source = match decision {
                WindowRefreshDecision::MaskFrozenAlpha => {
                    "Window · synthetic alpha on frozen pixels"
                }
                WindowRefreshDecision::ReplaceWithIndependent => {
                    "Window · synthetic independent pixels"
                }
                WindowRefreshDecision::KeepFrozen => return false,
            };
            // Same path as edits: pending OCR cancelled, copy payload cleared,
            // preview revision advanced and old tiles discarded/rebuilt.
            self.changed(cx);
        }
        // Failure leaves the usable frozen screenshot and its edits undisturbed.
        accepted
    }
}

/// macOS uses the same CoreGraphics operation as the Swift source. Linux keeps
/// the explicitly approximate deterministic portable sampler for synthetic QA.
fn prepare_refresh(
    plan: WindowRefreshPlan,
    independent: bellobox_core::screenshot::ScreenshotDocument,
    cancellation: Arc<AtomicBool>,
) -> bellobox_core::screenshot::window_refresh::WindowRefreshResult<Option<PreparedWindowRefresh>> {
    #[cfg(target_os = "macos")]
    {
        use bello_platform::native_capture::{
            AlphaMaskError, AlphaMaskInput, CaptureCancellation, mask_image_alpha,
        };
        use bellobox_core::screenshot::{ScreenshotDocument, window_refresh::WindowRefreshError};
        plan.prepare_with_alpha_mask(independent, |frozen, shape, _| {
            let (width, height) = frozen.dimensions();
            let (sw, sh) = shape.dimensions();
            let png = mask_image_alpha(
                AlphaMaskInput {
                    width,
                    height,
                    rgba: frozen.rgba(),
                },
                AlphaMaskInput {
                    width: sw,
                    height: sh,
                    rgba: shape.rgba(),
                },
                CaptureCancellation::from_flag(cancellation.clone()),
            )
            .map_err(|error| match error {
                AlphaMaskError::Cancelled => WindowRefreshError::Cancelled,
                AlphaMaskError::InputTooLarge | AlphaMaskError::OutputTooLarge => {
                    WindowRefreshError::OutputTooLarge
                }
                AlphaMaskError::IncompatibleImages => WindowRefreshError::IncompatibleImages,
                _ => WindowRefreshError::MaskFailed,
            })?;
            ScreenshotDocument::from_png(&png).map_err(|_| WindowRefreshError::MaskFailed)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cancellation;
        plan.prepare(independent)
    }
}

#[cfg(test)]
mod tests;
