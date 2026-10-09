//! Memory-only, window-owned HTTP review and explicit dispatch lifecycle.
use crate::{
    session::{JobToken, SessionJobs},
    shutdown,
};
use bellobox_core::developer::http_request::{
    HttpInspectionResult, HttpRequestDraft, MAX_DRAFT_BYTES, PreparedHttpRequest,
};
use gpui::{Context, Subscription};
use std::sync::atomic::Ordering;

pub(crate) struct HttpSession {
    pub draft: HttpRequestDraft,
    pub result: Option<HttpInspectionResult>,
    pub error: Option<String>,
    pub status: String,
    pub revision: u64,
    pub busy: bool,
    running: bool,
    retired: bool,
    rejected: bool,
    jobs: SessionJobs,
    desired: Option<JobToken>,
    ready: Option<JobToken>,
    completion: Option<shutdown::Close>,
    _shutdown: Subscription,
    #[cfg(test)]
    publication_held: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl HttpSession {
    pub fn new(cx: &mut Context<Self>) -> Self {
        shutdown::registry(cx);
        let subscription = cx.observe_global::<shutdown::Shutdown>(|s, cx| {
            if shutdown::requested(cx) {
                s.retire(cx);
            }
        });
        cx.on_release(|s: &mut Self, _| s.invalidate()).detach();
        Self {
            draft: HttpRequestDraft::default(),
            result: None,
            error: None,
            status: "Review the request fields, then choose Send.".into(),
            revision: 0,
            busy: false,
            running: false,
            retired: false,
            rejected: false,
            jobs: SessionJobs::default(),
            desired: None,
            ready: None,
            completion: None,
            _shutdown: subscription,
            #[cfg(test)]
            publication_held: Default::default(),
        }
    }
    pub fn can_send(&self) -> bool {
        !self.retired && !self.running && !self.rejected && !self.draft.url.is_empty()
    }
    pub fn physical_running(&self) -> bool {
        self.running
    }
    pub fn can_copy(&self) -> bool {
        !self.retired
            && !self.rejected
            && !self.busy
            && self.ready.is_some_and(|t| self.jobs.accepts(t))
            && self.result.is_some()
    }
    fn invalidate(&mut self) {
        self.jobs.cancel();
        self.desired = None;
        self.ready = None;
        self.result = None;
        self.error = None;
        self.busy = false;
        if let Some(completion) = &self.completion {
            completion.close();
        }
        if let Some(next) = self.revision.checked_add(1) {
            self.revision = next;
        } else {
            self.retired = true;
            self.error = Some("This HTTP session expired. Open New Window to continue.".into());
        }
    }
    pub fn set_draft(&mut self, draft: HttpRequestDraft, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.invalidate();
        if draft.byte_len() > MAX_DRAFT_BYTES {
            self.rejected = true;
            self.error =
                Some("Request fields exceed 500,000 UTF-8 bytes. Nothing was truncated.".into());
        } else {
            self.draft = draft;
            self.rejected = false;
            self.status = "Review the request fields, then choose Send.".into();
        }
        cx.notify();
    }
    pub fn reject(&mut self, error: String, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.invalidate();
        self.rejected = true;
        self.error = Some(error);
        self.status.clear();
        cx.notify();
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.invalidate();
        self.status = if self.running {
            "Cancelling; waiting for the request to stop."
        } else {
            "Cancelled."
        }
        .into();
        cx.notify();
    }
    pub fn retire(&mut self, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.cancel(cx);
        self.retired = true;
        // Draft/request/response owners are never written to settings/history.
        self.draft = HttpRequestDraft::default();
        cx.notify();
    }
    pub fn send(&mut self, cx: &mut Context<Self>) {
        if !self.can_send() || shutdown::requested(cx) {
            return;
        }
        let request = match self.draft.prepare() {
            Ok(request) => request,
            Err(error) => {
                self.reject(error, cx);
                return;
            }
        };
        self.dispatch(request, cx);
    }
    fn dispatch(&mut self, request: PreparedHttpRequest, cx: &mut Context<Self>) {
        self.invalidate();
        if self.retired {
            return;
        }
        let token = self.jobs.begin();
        let cancel = self.jobs.cancellation();
        let shutdown_cancel = cancel.clone();
        let Some(permit) =
            shutdown::admit(cx, move || shutdown_cancel.store(true, Ordering::Release))
        else {
            self.retire(cx);
            return;
        };
        self.desired = Some(token);
        self.running = true;
        self.busy = true;
        self.status = "Sending request…".into();
        let mut task = permit.spawn(cx, move || {
            crate::transport::http_request::send(request, &cancel)
        });
        self.completion = Some(task.close_handle());
        #[cfg(test)]
        let held = self.publication_held.clone();
        cx.spawn(async move |this, cx| {
            #[cfg(test)]
            while held.load(Ordering::Acquire) {
                cx.background_executor().timer(std::time::Duration::from_millis(10)).await;
            }
            task.ready().await;
            let _ = this.update(cx, |s, cx| {
                if s.desired != Some(token) || !s.jobs.accepts(token) || s.retired || shutdown::requested(cx) { return; }
                let result = match task.take() {
                    Some(result) => result,
                    None if task.aborted() => Err("The HTTP worker stopped unexpectedly. Edit the request and try Send again.".into()),
                    None => return,
                };
                s.desired = None;
                s.busy = false;
                match result {
                    Ok(result) => { s.status = result.status().into(); s.result = Some(result); s.ready = Some(token); }
                    Err(error) => { s.error = Some(error); s.status.clear(); }
                }
                s.revision = s.revision.checked_add(1).unwrap_or_else(|| { s.retired = true; 0 });
                cx.notify();
            });
            // Even accepted publication cannot release physical admission until
            // the terminal owner/runtime/worker have retired. Cancel/edit/close
            // close this slot immediately but do not pretend the worker is gone.
            task.close();
            task.ready().await;
            let _ = this.update(cx, |s, cx| {
                s.running = false;
                s.completion = None;
                if !s.busy && s.result.is_none() && s.error.is_none() && !s.retired {
                    s.status = "Request stopped. Review the fields before sending again.".into();
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
#[cfg(test)]
mod tests;
