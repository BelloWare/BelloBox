//! Window-owned literal drafts and one cancellable physical regex worker.
use crate::session::{JobToken, SessionJobs};
use bellobox_core::developer::regex_inspection::{Inspection, Mode, Options, inspect};
use gpui::Context;
use std::{sync::Arc, time::Duration};

pub(crate) struct RegexSession {
    inputs: [Arc<str>; 3],
    pub options: Options,
    pub mode: Mode,
    pub result: Option<Inspection>,
    pub error: Option<String>,
    pub status: String,
    pub busy: bool,
    pub revision: u64,
    jobs: SessionJobs,
    desired: Option<JobToken>,
    ready: Option<JobToken>,
    running: bool,
    rejected: bool,
    retired: bool,
    #[cfg(test)]
    probe: WorkerProbe,
}
impl RegexSession {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let s = Self {
            inputs: [Arc::from(""), Arc::from(""), Arc::from("")],
            options: Options::default(),
            mode: Mode::default(),
            result: None,
            error: None,
            status: String::new(),
            busy: false,
            revision: 0,
            jobs: SessionJobs::default(),
            desired: None,
            ready: None,
            running: false,
            rejected: false,
            retired: false,
            #[cfg(test)]
            probe: WorkerProbe::default(),
        };
        cx.notify();
        s
    }
    pub fn has_current_result(&self) -> bool {
        !self.retired
            && !self.rejected
            && !self.busy
            && self.error.is_none()
            && self.ready.is_some_and(|t| self.jobs.accepts(t))
            && self.result.is_some()
    }
    pub fn can_copy(&self) -> bool {
        self.has_current_result()
            && self
                .result
                .as_ref()
                .is_some_and(|r| !r.output(self.mode).is_empty())
    }
    pub fn set_drafts(
        &mut self,
        text: String,
        pattern: String,
        replacement: String,
        cx: &mut Context<Self>,
    ) {
        if self.retired {
            return;
        }
        if let Err(error) =
            bellobox_core::developer::regex_inspection::validate(&text, &pattern, &replacement)
        {
            self.reject(error, cx);
            return;
        }
        self.inputs = [Arc::from(text), Arc::from(pattern), Arc::from(replacement)];
        self.rejected = false;
        self.changed(cx);
    }
    pub fn set_options(&mut self, options: Options, cx: &mut Context<Self>) {
        if self.retired || self.options == options {
            return;
        }
        self.options = options;
        self.changed(cx);
    }
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.retired || self.mode == mode {
            return;
        }
        self.mode = mode;
        self.advance_revision();
        cx.notify();
    }
    pub fn can_chain(&self) -> bool {
        self.can_copy()
            && self.mode != Mode::Matches
            && self.result.as_ref().is_some_and(|r| {
                let output = r.output(self.mode);
                output.len() <= bellobox_core::developer::regex_inspection::INPUT_LIMIT
                    && output != self.inputs[0].as_ref()
            })
    }
    fn clear_result(&mut self) {
        self.result = None;
        self.ready = None;
        self.error = None;
        self.status.clear();
        self.advance_revision();
    }
    fn advance_revision(&mut self) {
        if let Some(revision) = self.revision.checked_add(1) {
            self.revision = revision;
        } else {
            self.jobs.cancel();
            self.desired = None;
            self.ready = None;
            self.result = None;
            self.busy = false;
            self.retired = true;
            self.error = Some("This regex session expired. Open New Window to continue.".into());
        }
    }
    pub fn reject(&mut self, error: String, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.jobs.cancel();
        self.desired = None;
        self.busy = false;
        self.rejected = true;
        self.clear_result();
        self.error = Some(error);
        cx.notify();
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.retired {
            self.changed(cx);
        }
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.jobs.cancel();
        self.desired = None;
        self.busy = false;
        self.clear_result();
        self.status = "Cancelled.".into();
        cx.notify();
    }
    pub fn suspend_composition(&mut self, cx: &mut Context<Self>) {
        self.cancel(cx);
        self.status = "Finish the current text composition to update matches.".into();
    }
    pub fn retire(&mut self, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.cancel(cx);
        self.retired = true;
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        let token = self.jobs.begin();
        self.clear_result();
        self.desired = None;
        self.busy = false;
        if self.retired {
            cx.notify();
            return;
        }
        if self.rejected {
            self.error =
                Some("A draft was rejected. Edit or replace that draft to continue.".into());
        } else if crate::tool_controls::source_input_is_idle(&self.inputs[0]) {
            // Swift schedules no work for whitespace-only input.
        } else if self.inputs[1].is_empty() {
            self.status = "Enter a regular expression to see live matches.".into();
        } else {
            self.desired = Some(token);
            self.busy = true;
            self.status = "Working locally…".into();
            self.start_worker(cx);
        }
        cx.notify();
    }
    fn start_worker(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(token)) = this.update(cx, |s, _| s.desired) else {
                    let _ = this.update(cx, |s, _| s.running = false);
                    break;
                };
                cx.background_executor()
                    .timer(Duration::from_millis(220))
                    .await;
                let request = this.update(cx, |s, _| {
                    (s.desired == Some(token) && s.jobs.accepts(token) && !s.retired)
                        .then(|| (s.inputs.clone(), s.options, s.jobs.cancellation()))
                });
                let (inputs, options, cancellation) = match request {
                    Ok(Some(r)) => r,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                #[cfg(test)]
                let probe = match this.update(cx, |s, _| s.probe.clone()) {
                    Ok(p) => p,
                    Err(_) => break,
                };
                #[cfg(test)]
                let executor = cx.background_executor().clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        #[cfg(test)]
                        {
                            use std::sync::atomic::Ordering::SeqCst;
                            probe.starts.fetch_add(1, SeqCst);
                            while probe.held.load(SeqCst) {
                                executor.timer(Duration::from_millis(10)).await;
                            }
                        }
                        inspect(&inputs[0], &inputs[1], &inputs[2], options, &cancellation)
                    })
                    .await;
                if this
                    .update(cx, |s, cx| s.publish(token, result, cx))
                    .is_err()
                {
                    break;
                }
                // A newer request waits for this physical worker to drain. Only
                // the latest desired drafts are cloned on the next iteration.
            }
        })
        .detach();
    }
    fn publish(
        &mut self,
        token: JobToken,
        result: Result<Inspection, String>,
        cx: &mut Context<Self>,
    ) {
        if self.desired != Some(token) || !self.jobs.accepts(token) || self.retired || self.rejected
        {
            return;
        }
        self.desired = None;
        self.busy = false;
        match result {
            Ok(result) => {
                self.status = format!(
                    "{} matches · portable Rust regex · detail ranges are UTF-16",
                    result.matches.len()
                );
                self.result = Some(result);
                self.ready = Some(token);
            }
            Err(error) => {
                self.status.clear();
                self.error = Some(error);
            }
        }
        self.advance_revision();
        cx.notify();
    }
}
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct WorkerProbe {
    pub held: Arc<std::sync::atomic::AtomicBool>,
    pub starts: Arc<std::sync::atomic::AtomicUsize>,
}
#[cfg(test)]
impl RegexSession {
    pub(crate) fn worker_probe(&self) -> WorkerProbe {
        self.probe.clone()
    }
}
#[cfg(test)]
mod tests;
