//! Compact explicit-send Copilot. The launcher retains physical retirement
//! observers separately, so replacing this preview cannot bypass close safety.
use super::*;
use crate::clock_copilot_worker::{RetirementGuard, Worker};
use bellobox_core::clock::copilot::{
    apply, protocol,
    session::{Outcome, Session},
};

pub(super) struct Copilot {
    pub draft: Entity<EditorView>,
    pub draft_focus: FocusHandle,
    controls: HashMap<Action, FocusHandle>,
    pub session: Session,
    pub worker: Worker,
    guards: Rc<RefCell<Vec<RetirementGuard>>>,
    notice: Option<String>,
    closed: bool,
    settings_path: std::path::PathBuf,
}
impl Copilot {
    pub fn new(window: &mut Window, cx: &mut Context<LauncherClockPreview>) -> Self {
        let draft = cx.new(|cx| {
            let mut editor = EditorView::new(String::new(), window, cx);
            let mut appearance = EditorAppearance::plain();
            appearance.font_size = 11.;
            appearance.line_height = 14.;
            editor.set_appearance(appearance, cx);
            editor.set_compact(true, cx);
            editor
        });
        let draft_focus = draft.read(cx).focus_handle(cx);
        Self {
            draft,
            draft_focus,
            controls: HashMap::new(),
            session: Session::default(),
            worker: Worker::default(),
            guards: Default::default(),
            notice: None,
            closed: false,
            settings_path: bellobox_core::settings::config_dir().join("settings.json"),
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
enum Action {
    Send,
    Retry,
    Cancel,
    Clear,
    Copy(u64),
    Apply(u64, u64),
}
impl Action {
    fn focus_key(&self) -> Self {
        match self {
            Self::Apply(id, _) => Self::Apply(*id, 0),
            other => other.clone(),
        }
    }
}
impl LauncherClockPreview {
    fn control_actions(&self) -> Vec<Action> {
        let mut actions = vec![Action::Send, Action::Retry, Action::Cancel, Action::Clear];
        for m in self.copilot.session.messages() {
            if m.role == protocol::Role::Assistant {
                actions.push(Action::Copy(m.id));
                if m.suggestion.as_ref().is_some_and(|s| {
                    apply::prepare(&self.session.planner, s, m.applied_parts, true, false)
                        .is_ok_and(|p| p.is_some())
                }) {
                    actions.push(Action::Apply(m.id, self.revision));
                }
            }
        }
        actions
    }
    pub(super) fn refresh_copilot_controls(&mut self, cx: &mut Context<Self>) {
        let actions: Vec<_> = self
            .control_actions()
            .iter()
            .map(Action::focus_key)
            .collect();
        self.copilot.controls.retain(|a, _| actions.contains(a));
        for action in actions {
            self.copilot
                .controls
                .entry(action)
                .or_insert_with(|| cx.focus_handle());
        }
    }
    pub(super) fn copilot_focus_order(&self) -> Vec<FocusHandle> {
        self.control_actions()
            .iter()
            .filter_map(|a| self.copilot.controls.get(&a.focus_key()).cloned())
            .collect()
    }
    pub(super) fn activate_copilot_control(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.key != "enter" {
            return false;
        }
        let action = self.control_actions().into_iter().find(|action| {
            self.copilot
                .controls
                .get(&action.focus_key())
                .is_some_and(|focus| focus.is_focused(window))
        });
        if let Some(action) = action {
            self.consume_enter_release = true;
            if !event.is_held {
                self.copilot_action(action, cx);
            }
            window.prevent_default();
            cx.stop_propagation();
            true
        } else {
            false
        }
    }
    #[cfg(test)]
    pub fn send_fixture(&mut self, path: &std::path::Path, question: &str, cx: &mut Context<Self>) {
        self.copilot.settings_path = path.to_owned();
        self.copilot
            .draft
            .update(cx, |e, cx| e.set_text(question.into(), cx));
        self.send_copilot(false, cx);
    }
    #[cfg(test)]
    pub fn invalidate_handoff_fixture(&mut self) {
        self.session.planner.instant = "+262142-12-31T23:59:59Z".parse().unwrap();
        self.session.planner.follows_now = false;
    }
    pub fn retain_guards(&mut self, guards: Rc<RefCell<Vec<RetirementGuard>>>) {
        self.copilot.guards = guards;
    }
    pub fn retire_copilot(&mut self, cx: &mut Context<Self>) {
        self.copilot.closed = true;
        self.cancel_copilot(cx);
    }
    pub fn cancel_copilot(&mut self, cx: &mut Context<Self>) {
        self.copilot.worker.cancel();
        self.copilot.session.cancel();
        cx.notify();
    }
    pub fn draft_composing(&self, window: &Window, cx: &App) -> bool {
        self.draft_focused(window, cx) && self.copilot.draft.read(cx).has_marked_text()
    }
    #[cfg(test)]
    pub fn draft_editor(&self) -> Entity<EditorView> {
        self.copilot.draft.clone()
    }
    pub fn draft_focused(&self, window: &Window, cx: &App) -> bool {
        self.copilot
            .draft
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
    }
    pub fn height(&self) -> f32 {
        PREVIEW_HEIGHT
            + if !self.copilot.session.messages().is_empty()
                || self.copilot.notice.is_some()
                || self.copilot.session.status_message().is_some()
            {
                104.
            } else {
                0.
            }
    }
    pub(super) fn install_copilot(&mut self, cx: &mut Context<Self>) {
        let draft = self.copilot.draft.clone();
        self._subscriptions
            .push(cx.subscribe(&draft, |_, _, _: &EditorEvent, cx| cx.notify()));
        cx.spawn(async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if let Some((generation, result)) = this.copilot.worker.poll() {
                            this.copilot.session.complete(generation, result);
                            cx.notify();
                        }
                        this.copilot
                            .guards
                            .borrow_mut()
                            .retain(RetirementGuard::physically_active);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
    pub(super) fn send_copilot(&mut self, retry: bool, cx: &mut Context<Self>) {
        if self.copilot.closed || crate::shutdown::requested(cx) {
            return;
        }
        if self.copilot.worker.busy() {
            self.copilot.notice = Some("Previous request is still stopping.".into());
            return;
        }
        let context = protocol::Context::from_planner(
            &self.session.planner,
            clock::current_time(),
            &local_zone().unwrap_or_else(|| "UTC".into()),
        );
        let context = match context {
            Ok(v) => v,
            Err(e) => {
                self.copilot.notice = Some(e);
                return;
            }
        };
        let config = Settings::load(&self.copilot.settings_path).and_then(|settings| {
            crate::transport::config_from_settings(
                settings,
                std::env::var("BELLOBOX_AI_PROVIDER").ok().as_deref(),
                std::env::var("BELLOBOX_AI_ENDPOINT").ok().as_deref(),
                std::env::var("BELLOBOX_AI_MODEL").ok().as_deref(),
            )
        });
        let key = std::env::var("BELLOBOX_AI_KEY").unwrap_or_default();
        let question = self.copilot.draft.read(cx).text().to_owned();
        let worker = &mut self.copilot.worker;
        let admit = |request: &protocol::CopilotRequest, generation| {
            let request =
                protocol::provider_request(config.as_ref().map_err(Clone::clone)?, &key, request)?;
            worker.start(
                generation,
                request,
                crate::shutdown::block_quit(
                    cx,
                    "Wait for the World Clock Copilot request to finish.",
                ),
            )
        };
        let accepted = if retry {
            self.copilot.session.retry(context, admit)
        } else {
            self.copilot.session.begin(&question, context, admit)
        };
        self.copilot.notice = accepted.as_ref().err().cloned();
        if accepted.is_ok() {
            if let Some(guard) = self.copilot.worker.guard() {
                let mut guards = self.copilot.guards.borrow_mut();
                guards.retain(RetirementGuard::physically_active);
                guards.push(guard);
            }
            if !retry {
                self.copilot
                    .draft
                    .update(cx, |e, cx| e.set_text(String::new(), cx));
            }
        }
    }
    fn copilot_action(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.copilot.closed || crate::shutdown::requested(cx) {
            return;
        }
        match action {
            Action::Send => self.send_copilot(false, cx),
            Action::Retry => self.send_copilot(true, cx),
            Action::Cancel => {
                self.copilot.worker.cancel();
                self.copilot.session.cancel();
                self.copilot.notice = None;
            }
            Action::Clear => {
                self.copilot.worker.cancel();
                self.copilot.session.clear();
                self.copilot.notice = None;
            }
            Action::Copy(id) => {
                if let Some(m) = self.copilot.session.messages().iter().find(|m| m.id == id) {
                    cx.write_to_clipboard(ClipboardItem::new_string(m.text.clone()));
                }
            }
            Action::Apply(id, revision) => {
                if revision != self.revision {
                    self.copilot.notice = Some("Planner changed. Review and Apply again.".into());
                } else if let Some(m) = self.copilot.session.messages().iter().find(|m| m.id == id)
                    && let Some(s) = &m.suggestion
                {
                    match apply::prepare(&self.session.planner, s, m.applied_parts, true, false) {
                        Ok(Some(prepared)) => {
                            let summary = prepared.plan.summary.clone();
                            match prepared.commit(&mut self.session.planner) {
                                Ok(parts) => {
                                    self.copilot.session.mark_applied(id, parts, &summary);
                                    self.session.displayed_day =
                                        self.session.planner.timeline().expect("prepared timeline");
                                    self.changed(Ok(()), cx);
                                    self.copilot.notice = None;
                                }
                                Err(e) => self.copilot.notice = Some(e),
                            }
                        }
                        Ok(None) => self.copilot.notice = Some("Already in effect.".into()),
                        Err(e) => self.copilot.notice = Some(e),
                    }
                }
            }
        }
        cx.notify();
    }
    fn copilot_button(
        &self,
        id: impl Into<ElementId>,
        label: &str,
        action: Action,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let focus = self.copilot.controls.get(&action.focus_key()).cloned();
        div()
            .id(id)
            .when_some(focus, |d, focus| d.track_focus(&focus))
            .px(px(5.))
            .rounded(px(4.))
            .bg(p.well)
            .cursor_pointer()
            .child(label.to_owned())
            .on_click(cx.listener(move |this, _, _, cx| this.copilot_action(action.clone(), cx)))
    }
    pub(super) fn copilot_view(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut transcript = div()
            .id("palette-copilot-transcript")
            .h(px(98.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(3.));
        for m in self.copilot.session.messages() {
            let mut row = div().child(format!(
                "{}: {}",
                if m.role == protocol::Role::User {
                    "You"
                } else {
                    "Copilot"
                },
                m.text
            ));
            if m.role == protocol::Role::Assistant {
                row = row.child(self.copilot_button(
                    ("palette-copy", m.id),
                    "Copy",
                    Action::Copy(m.id),
                    p,
                    cx,
                ));
                if let Some(s) = &m.suggestion {
                    match apply::prepare(&self.session.planner, s, m.applied_parts, true, false) {
                        Ok(Some(plan)) => {
                            row = row.child(plan.plan.summary).child(self.copilot_button(
                                ("palette-apply", m.id),
                                "Apply time",
                                Action::Apply(m.id, self.revision),
                                p,
                                cx,
                            ));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            row = row.child(e);
                        }
                    }
                    if s.parts()
                        .contains(bellobox_core::clock::copilot::Parts::LOCATIONS)
                    {
                        row = row.child("Open World Clock to apply locations.");
                    }
                }
                if let Some(issue) = &m.issue {
                    row = row.child(issue.clone());
                }
            }
            transcript = transcript.child(row);
        }
        let notice = self
            .copilot
            .notice
            .clone()
            .or_else(|| match self.copilot.session.outcome() {
                Outcome::Failed(e) => Some(e.clone()),
                _ => self.copilot.session.status_message().map(str::to_owned),
            });
        if let Some(notice) = notice {
            transcript = transcript.child(notice);
        }
        if self.copilot.worker.busy() {
            transcript = transcript.child(if self.copilot.session.is_busy() {
                "Thinking…"
            } else {
                "Stopping…"
            });
        }
        let mut result = div()
            .id("palette-copilot")
            .text_size(px(10.))
            .flex()
            .flex_col()
            .min_h(px(COPILOT_HEIGHT))
            .gap(px(3.));
        if self.height() > PREVIEW_HEIGHT {
            result = result.child(transcript);
        }
        result
            .child(div().h(px(18.)).child(self.copilot.draft.clone()))
            .child(
                div()
                    .flex()
                    .gap(px(3.))
                    .child(self.copilot_button("palette-send", "Send", Action::Send, p, cx))
                    .child(self.copilot_button("palette-retry", "Retry", Action::Retry, p, cx))
                    .child(self.copilot_button("palette-cancel", "Cancel", Action::Cancel, p, cx))
                    .child(self.copilot_button("palette-clear", "Clear", Action::Clear, p, cx)),
            )
    }
}

#[cfg(test)]
mod tests;
