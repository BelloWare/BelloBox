//! Production-compiled backend-injected refresh through the real editor/OCR path.
//! Generated pixels exist only in explicit DEBUG/test fixtures. Native gates stay shut.
use super::ScreenshotEditor;
use crate::session::{JobToken, SessionJobs};
use bellobox_core::screenshot::window_refresh::{
    PreparedWindowRefresh, WindowRefreshContext, WindowRefreshDecision, WindowRefreshPlan,
};
#[cfg(any(debug_assertions, test))]
use bellobox_core::screenshot::{
    window::{FrozenWindowCommit, synthetic_independent_window},
    window_refresh::{OcclusionLayers, OcclusionRow, WindowRefreshSource, decide_window_refresh},
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
    input: Input,
    #[cfg(test)]
    pub(super) disposal_gate: Option<DisposalGate>,
}
enum Input {
    #[cfg(any(debug_assertions, test))]
    Fixture(u32),
    Supplied(super::window_workflow::Acquisition),
}
impl Request {
    pub fn supplied(
        context: WindowRefreshContext,
        decision: WindowRefreshDecision,
        boundary: Arc<AtomicBool>,
        acquisition: super::window_workflow::Acquisition,
    ) -> Self {
        Self {
            context,
            decision,
            boundary,
            input: Input::Supplied(acquisition),
            #[cfg(test)]
            disposal_gate: None,
        }
    }

    #[cfg(any(debug_assertions, test))]
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
            input: Input::Fixture(candidate.window_id),
            #[cfg(test)]
            disposal_gate: None,
        })
    }
}

pub(super) struct Host {
    context: WindowRefreshContext,
    decision: WindowRefreshDecision,
    boundary: Arc<AtomicBool>,
    jobs: SessionJobs,
    publication: Option<super::window_workflow::Publication>,
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
            publication: None,
        };
        let token = host.jobs.begin();
        let cancellation = host.jobs.cancellation();
        if host.boundary.load(Ordering::Acquire) || !self.inline_current(cx) {
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
        // Count physical acquisition, mask work and completion disposal even when
        // the selector/editor are closed or the logical job is superseded.
        let inline_work = self.begin_inline_work(cx);
        if self.inline.is_some() && inline_work.is_none() {
            return;
        }
        self.window_refresh = Some(host);
        let input = request.input;
        #[cfg(test)]
        let disposal_gate = request.disposal_gate;
        let task = cx.background_executor().spawn(async move {
            super::caught_capture(|| {
                if boundary.load(Ordering::Acquire) || cancellation.load(Ordering::Acquire) {
                    return Ok((None, None));
                }
                let (independent, publication) = match input {
                    #[cfg(any(debug_assertions, test))]
                    Input::Fixture(window_id) => (
                        synthetic_independent_window(window_id, &cancellation)
                            .map_err(|e| e.to_string())?,
                        None,
                    ),
                    Input::Supplied(acquisition) => {
                        let (image, publication) = acquisition.run(cancellation.clone())?;
                        (image, Some(publication))
                    }
                };
                if boundary.load(Ordering::Acquire) {
                    return Ok((None, None));
                }
                Ok((
                    prepare_refresh(plan, independent, cancellation).map_err(|e| e.to_string())?,
                    publication,
                ))
            })
        });
        cx.spawn(async move |this, cx| {
            let (prepared, publication) = task.await.unwrap_or((None, None));
            let mut original = None;
            let retained = prepared.clone();
            let _ = this.update(cx, |this, cx| {
                original = Some(this.session.render_snapshot());
                if let Some(host) = this
                    .window_refresh
                    .as_mut()
                    .filter(|host| host.jobs.accepts(token))
                {
                    host.publication = publication;
                }
                this.accept_window_refresh(token, prepared, cx);
            });
            // The retained immutable pixels prevent final heavy destruction in
            // apply(), rejection or a dead-entity delivery on the UI executor.
            cx.background_executor()
                .spawn(async move {
                    #[cfg(test)]
                    if let Some(gate) = disposal_gate {
                        gate.wait().await;
                    }
                    drop((retained, original));
                })
                .await;
            let _ = cx.update(|cx| super::main_area::finish_editor_work(inline_work, cx));
        })
        .detach();
    }

    fn accept_window_refresh(
        &mut self,
        token: JobToken,
        prepared: Option<PreparedWindowRefresh>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.inline_current(cx) {
            self.cancel_inline_owner(cx);
            return false;
        }
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
            && host
                .publication
                .as_ref()
                .is_none_or(|publication| publication.is_current())
            && prepared.is_some_and(|prepared| {
                prepared
                    .apply(&mut self.session, host.context, &host.boundary)
                    .unwrap_or(false)
            });
        let decision = host.decision;
        host.jobs.cancel();
        if accepted {
            self.source = match decision {
                WindowRefreshDecision::MaskFrozenAlpha => "Window · alpha on frozen pixels",
                WindowRefreshDecision::ReplaceWithIndependent => "Window · independent pixels",
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

/// Deterministic test suspension inside the real background-disposal future.
/// No production delay, polling timer or native resource is introduced.
#[cfg(test)]
#[derive(Clone, Default)]
pub(super) struct DisposalGate(Arc<DisposalState>);
#[cfg(test)]
#[derive(Default)]
struct DisposalState {
    arrived: AtomicBool,
    released: AtomicBool,
    waker: std::sync::Mutex<Option<std::task::Waker>>,
}
#[cfg(test)]
impl DisposalGate {
    pub fn arrived(&self) -> bool {
        self.0.arrived.load(Ordering::Acquire)
    }
    pub fn release(&self) {
        self.0.released.store(true, Ordering::Release);
        if let Some(waker) = self.0.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
    async fn wait(self) {
        std::future::poll_fn(|cx| {
            let mut waker = self.0.waker.lock().unwrap();
            self.0.arrived.store(true, Ordering::Release);
            if self.0.released.load(Ordering::Acquire) {
                std::task::Poll::Ready(())
            } else {
                *waker = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        })
        .await
    }
}
