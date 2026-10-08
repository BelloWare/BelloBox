//! Dedicated-window conversation. Explicit Send is the only network entry point.
use super::*;
use bellobox_core::clock::copilot::{apply, protocol, session::Session};

pub(super) struct Copilot {
    pub draft: Entity<EditorView>,
    pub visible: bool,
    session: Session,
    worker: super::copilot_worker::Worker,
    closed: bool,
    notice: Option<String>,
}
impl Copilot {
    pub fn new(window: &mut Window, cx: &mut Context<WorldClock>) -> Self {
        Self {
            draft: plain_editor(
                String::new(),
                14.,
                crate::theme::for_window(window),
                window,
                cx,
            ),
            visible: false,
            session: Session::default(),
            worker: Default::default(),
            closed: false,
            notice: None,
        }
    }
}
impl WorldClock {
    pub(super) fn install_copilot_lifecycle(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::shutdown::admit_quit_window(window, cx);
        let close_owner = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let allowed = close_owner
                .update(cx, |this, cx| this.allow_copilot_close(cx))
                .unwrap_or(true);
            allowed && crate::shutdown::allow_close(window, cx)
        });
        let owner = window.window_handle();
        let weak = cx.weak_entity();
        self._subscriptions.push(cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner) {
                let _ = weak.update(cx, |this, _| {
                    this.copilot.closed = true;
                    this.copilot.worker.cancel();
                    this.copilot.session.clear();
                });
            }
        }));
        let draft = self.copilot.draft.clone();
        self._subscriptions
            .push(cx.subscribe(&draft, |_, _, _: &EditorEvent, cx| cx.notify()));
        cx.spawn(async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.copilot.closed {
                            return false;
                        }
                        if let Some((generation, reply)) = this.copilot.worker.poll() {
                            this.copilot.session.complete(generation, reply);
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }
    // Native close and any future owned Close action must use this gate before
    // shutdown::allow_close. Physical work outlives presentation cancellation.
    fn allow_copilot_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.copilot.worker.physically_active() {
            self.copilot.worker.cancel();
            self.copilot.session.cancel();
            self.copilot.visible = true;
            self.copilot.notice =
                Some("Stopping the Copilot request. Close again after it finishes.".into());
            cx.notify();
            false
        } else {
            true
        }
    }
    pub(super) fn reset_copilot(&mut self, cx: &mut Context<Self>) {
        self.copilot.worker.cancel();
        self.copilot.session.clear();
        self.copilot.notice = None;
        self.copilot
            .draft
            .update(cx, |e, cx| e.set_text(String::new(), cx));
    }
    fn send_copilot(
        &mut self,
        retry: bool,
        explicit_question: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if self.copilot.closed || crate::shutdown::requested(cx) {
            return;
        }
        if self.copilot.worker.busy() {
            self.copilot.notice = Some("The previous request is still running or stopping.".into());
            return;
        }
        let context = protocol::Context::from_planner(
            &self.planner,
            clock::current_time(),
            &local_zone().unwrap_or_else(|| "UTC".into()),
        );
        let context = match context {
            Ok(v) => v,
            Err(e) => {
                let _: Result<(), String> = self.copilot.session.reject(e.clone());
                self.copilot.notice = Some(e);
                return;
            }
        };
        let question = explicit_question
            .unwrap_or_else(|| self.copilot.draft.read(cx).text())
            .to_owned();
        let config = Settings::load(&self.settings_path).and_then(|settings| {
            crate::transport::config_from_settings(
                settings,
                std::env::var("BELLOBOX_AI_PROVIDER").ok().as_deref(),
                std::env::var("BELLOBOX_AI_ENDPOINT").ok().as_deref(),
                std::env::var("BELLOBOX_AI_MODEL").ok().as_deref(),
            )
        });
        let key = std::env::var("BELLOBOX_AI_KEY").unwrap_or_default();
        let worker = &mut self.copilot.worker;
        let admit = |request: &protocol::CopilotRequest, generation| {
            let request =
                protocol::provider_request(config.as_ref().map_err(Clone::clone)?, &key, request)?;
            let blocker = crate::shutdown::block_quit(
                cx,
                "Wait for the World Clock Copilot request to finish.",
            );
            worker.start(generation, request, blocker)
        };
        let accepted = if retry {
            self.copilot.session.retry(context, admit)
        } else {
            self.copilot.session.begin(&question, context, admit)
        };
        self.copilot.notice = accepted.as_ref().err().cloned();
        if accepted.is_ok() && !retry && explicit_question.is_none() {
            self.copilot
                .draft
                .update(cx, |e, cx| e.set_text(String::new(), cx));
        }
    }
    pub(super) fn copilot_action(
        &mut self,
        action: &Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if (self.copilot.closed || crate::shutdown::requested(cx))
            && matches!(
                action,
                Action::Copilot
                    | Action::CopilotSend
                    | Action::CopilotRetry
                    | Action::CopilotCancel
                    | Action::CopilotClear
                    | Action::CopilotSettings
                    | Action::CopilotPrompt(_)
                    | Action::CopilotApply(_, _)
                    | Action::CopilotCopy(_)
            )
        {
            return true;
        }
        match action {
            Action::Copilot => {
                self.copilot.visible = !self.copilot.visible;
                if self.copilot.visible {
                    self.copilot.draft.read(cx).focus_handle(cx).focus(window);
                } else {
                    self.copilot.worker.cancel();
                    self.copilot.session.cancel();
                    self.focus.focus(window);
                }
            }
            Action::CopilotSend => self.send_copilot(false, None, cx),
            Action::CopilotRetry => self.send_copilot(true, None, cx),
            Action::CopilotCancel => {
                self.copilot.worker.cancel();
                self.copilot.session.cancel();
                self.copilot.notice = None;
            }
            Action::CopilotClear => {
                self.copilot.worker.cancel();
                self.copilot.session.clear();
                self.copilot.notice = None;
            }
            Action::CopilotSettings => crate::settings_ui::open(cx),
            Action::CopilotPrompt(question) => self.send_copilot(false, Some(question), cx),
            Action::CopilotCopy(id) => {
                if let Some(m) = self.copilot.session.messages().iter().find(|m| m.id == *id) {
                    cx.write_to_clipboard(ClipboardItem::new_string(m.text.clone()));
                    self.copilot.notice = Some("Answer copied.".into());
                }
            }
            Action::CopilotApply(id, revision) => {
                if *revision != self.planner_revision {
                    self.copilot.notice = Some(
                        "The planner changed. Review the updated suggestion, then Apply again."
                            .into(),
                    );
                } else if let Some(message) =
                    self.copilot.session.messages().iter().find(|m| m.id == *id)
                    && let Some(suggestion) = &message.suggestion
                {
                    match apply::prepare(
                        &self.planner,
                        suggestion,
                        message.applied_parts,
                        true,
                        true,
                    ) {
                        Ok(Some(prepared)) => {
                            let summary = prepared.plan.summary.clone();
                            let issue = prepared.issue.clone();
                            match prepared.commit(&mut self.planner) {
                                Ok(parts) => {
                                    self.copilot.session.mark_applied(*id, parts, &summary);
                                    self.changed_in_day(Ok(()), false, cx);
                                    if parts
                                        .contains(bellobox_core::clock::copilot::Parts::LOCATIONS)
                                    {
                                        self.persist();
                                    }
                                    self.copilot.notice = self.error.as_ref().map(|error| format!("Applied in this window, but preferences were not saved: {error}")).or(issue);
                                }
                                Err(e) => self.copilot.notice = Some(e),
                            }
                        }
                        Ok(None) => {
                            self.copilot.notice =
                                Some("This suggestion is already in effect.".into())
                        }
                        Err(e) => self.copilot.notice = Some(e),
                    }
                }
            }
            _ => return false,
        }
        cx.notify();
        true
    }
    pub(super) fn copilot_actions(&self) -> Vec<Action> {
        if !self.copilot.visible {
            return vec![];
        }
        let mut actions = vec![
            Action::CopilotSend,
            Action::CopilotRetry,
            Action::CopilotCancel,
            Action::CopilotClear,
            Action::CopilotSettings,
            Action::CopilotPrompt("Find a slot today that works for everyone".into()),
        ];
        for message in self.copilot.session.messages() {
            if message.role == protocol::Role::Assistant {
                actions.push(Action::CopilotCopy(message.id));
                if message.suggestion.as_ref().is_some_and(|suggestion| {
                    apply::prepare(&self.planner, suggestion, message.applied_parts, true, true)
                        .is_ok_and(|plan| plan.is_some())
                }) {
                    actions.push(Action::CopilotApply(message.id, self.planner_revision));
                }
            }
        }
        actions
    }
    pub(super) fn copilot_action_enabled(&self, action: &Action, cx: &App) -> bool {
        match action {
            Action::CopilotSend => {
                !self.copilot.worker.busy() && !self.copilot.draft.read(cx).text().trim().is_empty()
            }
            Action::CopilotPrompt(_) => !self.copilot.worker.busy(),
            Action::CopilotRetry => !self.copilot.worker.busy() && self.copilot.session.can_retry(),
            Action::CopilotCancel => self.copilot.session.is_busy(),
            _ => true,
        }
    }
    fn copilot_notices(&self) -> Vec<String> {
        let mut notices = Vec::new();
        if let Some(notice) = self.copilot.notice.as_ref() {
            notices.push(notice.clone());
        }
        if notices.is_empty()
            && let Some(status) = self.copilot.session.status_message()
        {
            notices.push(status.to_owned());
        }
        if let bellobox_core::clock::copilot::session::Outcome::Failed(error) =
            self.copilot.session.outcome()
            && !notices.contains(error)
        {
            notices.push(error.clone());
        }
        notices
    }
    pub(super) fn copilot_view(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut transcript = div()
            .id("clock-copilot-transcript")
            .max_h(px(240.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(8.));
        for message in self.copilot.session.messages() {
            let mut row = div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(8.))
                .bg(p.well)
                .rounded(px(6.))
                .child(div().text_color(p.secondary).child(
                    if message.role == protocol::Role::User {
                        "You"
                    } else {
                        "Copilot"
                    },
                ))
                .child(message.text.clone());
            if let Some(issue) = &message.issue {
                row = row.child(div().text_color(p.secondary).child(issue.clone()));
            }
            if message.role == protocol::Role::Assistant {
                row = row.child(self.button(
                    ("copilot-copy", message.id),
                    "Copy",
                    Action::CopilotCopy(message.id),
                    false,
                    p,
                    cx,
                ));
                if let Some(suggestion) = &message.suggestion {
                    match apply::prepare(
                        &self.planner,
                        suggestion,
                        message.applied_parts,
                        true,
                        true,
                    ) {
                        Ok(Some(prepared)) => {
                            row = row.child(prepared.plan.summary.clone());
                            if let Some(issue) = prepared.issue {
                                row = row.child(issue);
                            }
                            row = row.child(self.button(
                                ("copilot-apply", message.id),
                                "Apply",
                                Action::CopilotApply(message.id, self.planner_revision),
                                true,
                                p,
                                cx,
                            ));
                        }
                        Ok(None) => {
                            row = row.child(if message.applied_parts.is_empty() {
                                "Already in effect"
                            } else {
                                "Applied"
                            })
                        }
                        Err(e) => row = row.child(e),
                    }
                }
            }
            transcript = transcript.child(row);
        }
        div().id("clock-copilot-panel").flex_none().max_h(px(380.)).overflow_y_scroll().flex().flex_col().gap(px(8.)).p(px(12.)).border_t_1().border_color(p.separator)
            .child(div().font_weight(FontWeight::SEMIBOLD).child("World Clock Copilot"))
            .child(div().text_sm().text_color(p.secondary).child("Send or a suggested question contacts your configured provider. Conversations stay in memory. Runtime provider overrides apply."))
            .child(transcript)
            .when(self.copilot.worker.busy(),|d|d.child(if !self.copilot.worker.physically_active(){"Receiving answer…"}else if self.copilot.session.is_busy(){"Thinking…"}else{"Stopping; waiting for the request to retire…"}))
            .children(self.copilot_notices().into_iter().map(|notice| div().child(notice)))
            .child(div().h(px(44.)).bg(p.well).child(self.copilot.draft.clone()))
            .child(div().flex().gap(px(8.))
                .child(self.button("copilot-send","Send",Action::CopilotSend,true,p,cx))
                .child(self.button("copilot-retry","Retry",Action::CopilotRetry,false,p,cx))
                .child(self.button("copilot-cancel","Cancel",Action::CopilotCancel,false,p,cx))
                .child(self.button("copilot-clear","Clear",Action::CopilotClear,false,p,cx))
                .child(self.button("copilot-settings","AI Settings",Action::CopilotSettings,false,p,cx)))
            .child(self.button("copilot-example","Ask: Find a slot today",Action::CopilotPrompt("Find a slot today that works for everyone".into()),false,p,cx))
    }
}

#[cfg(test)]
mod tests;
