//! Ephemeral, bounded conversation state. Hosts own drafts, transport and cancellation.
//! Admission must finish before a user message is appended or a draft is cleared.
use super::{
    Parts, Suggestion,
    protocol::{self, Context, CopilotRequest, Reply, Role, Turn},
};

pub const TRANSCRIPT_LIMIT: usize = 40;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: u64,
    pub role: Role,
    pub text: String,
    pub suggestion: Option<Suggestion>,
    pub issue: Option<String>,
    pub applied_parts: Parts,
}
impl Message {
    pub fn is_fully_applied(&self) -> bool {
        self.suggestion
            .as_ref()
            .is_some_and(|s| !s.parts().is_empty() && self.applied_parts.contains(s.parts()))
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Outcome {
    #[default]
    None,
    Answered,
    Failed(String),
    Cancelled,
}
/// The admitted payload can be a validated provider request or an offline fixture.
pub struct Started<T> {
    pub generation: u64,
    pub request: CopilotRequest,
    pub admitted: T,
}
#[derive(Debug, Default)]
pub struct Session {
    messages: Vec<Message>,
    outcome: Outcome,
    status_message: Option<String>,
    pending_question: Option<String>,
    active: Option<u64>,
    generation: u64,
    next_message: u64,
}
impl Session {
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
    pub fn is_busy(&self) -> bool {
        self.active.is_some()
    }
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }
    pub fn can_retry(&self) -> bool {
        !self.is_busy()
            && self.pending_question.is_some()
            && matches!(self.outcome, Outcome::Failed(_) | Outcome::Cancelled)
    }
    pub fn begin<T>(
        &mut self,
        question: &str,
        context: Context,
        admit: impl FnOnce(&CopilotRequest, u64) -> Result<T, String>,
    ) -> Result<Started<T>, String> {
        if self.is_busy() {
            return Err("Wait for the current answer or cancel it first.".into());
        }
        let question = match protocol::validate_question(question) {
            Ok(q) => q,
            Err(e) => return self.reject(e),
        };
        let request = CopilotRequest {
            question,
            context,
            history: self.history(false),
        };
        self.start(request, false, admit)
    }
    pub fn retry<T>(
        &mut self,
        context: Context,
        admit: impl FnOnce(&CopilotRequest, u64) -> Result<T, String>,
    ) -> Result<Started<T>, String> {
        if !self.can_retry() {
            return Err("There is no unanswered question to retry.".into());
        }
        let request = CopilotRequest {
            question: self.pending_question.clone().expect("checked retry"),
            context,
            history: self.history(true),
        };
        self.start(request, true, admit)
    }
    fn history(&self, retry: bool) -> Vec<Turn> {
        let end = self.messages.len().saturating_sub(usize::from(retry));
        self.messages[..end]
            .iter()
            .rev()
            .take(protocol::HISTORY_TURNS)
            .rev()
            .map(|m| Turn {
                role: m.role,
                text: m.text.clone(),
            })
            .collect()
    }
    fn start<T>(
        &mut self,
        request: CopilotRequest,
        retry: bool,
        admit: impl FnOnce(&CopilotRequest, u64) -> Result<T, String>,
    ) -> Result<Started<T>, String> {
        if let Err(e) = protocol::user_prompt(&request) {
            return self.reject(e);
        }
        let Some(generation) = self.generation.checked_add(1) else {
            return self.reject("Session generation limit reached.".into());
        };
        // Reserve IDs for the user and answer before admitting any work.
        if self.next_message.checked_add(2).is_none() {
            return self.reject("Session message limit reached.".into());
        }
        // Admission may start a physical worker. Never reuse its generation,
        // even if admission subsequently fails.
        self.generation = generation;
        let admitted = match admit(&request, generation) {
            Ok(value) => value,
            Err(e) => return self.reject(e),
        };
        if !retry {
            self.append(Role::User, request.question.clone(), None, None);
        }
        self.pending_question = Some(request.question.clone());
        self.active = Some(generation);
        self.outcome = Outcome::None;
        self.status_message = None;
        Ok(Started {
            generation,
            request,
            admitted,
        })
    }
    pub fn reject<T>(&mut self, error: String) -> Result<T, String> {
        if self.is_busy() {
            return Err(error);
        }
        self.pending_question = None;
        self.outcome = Outcome::Failed(error.clone());
        self.status_message = None;
        Err(error)
    }
    /// Returns false for a late result after Cancel, Clear or a newer request.
    pub fn complete(&mut self, generation: u64, result: Result<Reply, String>) -> bool {
        if self.active != Some(generation) {
            return false;
        }
        self.active = None;
        match result {
            Ok(reply) => {
                self.append(
                    Role::Assistant,
                    reply.answer,
                    reply.suggestion,
                    reply.suggestion_issue,
                );
                self.outcome = Outcome::Answered;
                self.pending_question = None;
            }
            Err(error) => self.outcome = Outcome::Failed(error),
        }
        true
    }
    pub fn cancel(&mut self) {
        if self.active.take().is_some() {
            self.outcome = Outcome::Cancelled;
            self.status_message = Some("Cancelled. Nothing was changed.".into());
        }
    }
    /// Draft text is deliberately external, so Clear cannot erase unsent input.
    pub fn clear(&mut self) {
        self.active = None;
        self.messages.clear();
        self.pending_question = None;
        self.outcome = Outcome::None;
        self.status_message = None;
        // Never reset generation/message IDs: old replies and buttons stay stale.
    }
    pub fn mark_applied(&mut self, id: u64, parts: Parts, summary: &str) -> bool {
        let Some(message) = self.messages.iter_mut().find(|m| m.id == id) else {
            return false;
        };
        let Some(suggestion) = &message.suggestion else {
            return false;
        };
        if parts.is_empty() || !suggestion.parts().contains(parts) {
            return false;
        }
        message.applied_parts |= parts;
        self.status_message = Some(format!("Applied: {summary}"));
        true
    }
    fn append(
        &mut self,
        role: Role,
        text: String,
        suggestion: Option<Suggestion>,
        issue: Option<String>,
    ) {
        self.next_message += 1;
        self.messages.push(Message {
            id: self.next_message,
            role,
            text,
            suggestion,
            issue,
            applied_parts: Parts::empty(),
        });
        if self.messages.len() > TRANSCRIPT_LIMIT {
            self.messages
                .drain(..self.messages.len() - TRANSCRIPT_LIMIT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Planner;
    fn context() -> Context {
        let p = Planner::default();
        Context::from_planner(&p, p.instant, "UTC").unwrap()
    }
    fn reply() -> Reply {
        Reply {
            answer: "Answer".into(),
            suggestion: None,
            suggestion_issue: None,
        }
    }
    #[test]
    fn rejection_does_not_append_and_clears_old_retry() {
        let mut s = Session::default();
        let run = s.begin("first", context(), |_, _| Ok(())).unwrap();
        s.complete(run.generation, Err("offline".into()));
        assert!(s.can_retry());
        assert!(
            s.begin("second", context(), |_, _| Err::<(), _>(
                "No provider".into()
            ))
            .is_err()
        );
        assert_eq!(s.messages().len(), 1);
        assert!(!s.can_retry());
        assert!(
            s.begin(&"x".repeat(2001), context(), |_, _| Ok(()))
                .is_err()
        );
        assert_eq!(s.messages().len(), 1);
    }
    #[test]
    fn failed_admission_generation_is_never_reused() {
        let mut s = Session::default();
        let mut rejected_generation = 0;
        assert!(
            s.begin("first", context(), |_, generation| {
                rejected_generation = generation;
                Err::<(), _>("Worker unavailable".into())
            })
            .is_err()
        );
        let next = s.begin("second", context(), |_, _| Ok(())).unwrap();
        assert_ne!(next.generation, rejected_generation);
        assert!(!s.complete(rejected_generation, Ok(reply())));
        assert_eq!(s.messages().len(), 1);
        assert!(s.complete(next.generation, Ok(reply())));
    }
    #[test]
    fn invalid_context_is_rejected_before_admission_or_append() {
        let mut s = Session::default();
        let mut invalid = context();
        invalid.locations.clear();
        assert!(
            s.begin("first", invalid, |_, _| -> Result<(), String> {
                panic!("invalid request reached admission")
            })
            .is_err()
        );
        assert!(s.messages().is_empty());
        assert!(!s.can_retry());
    }
    #[test]
    fn retry_keeps_question_single_and_excludes_it_from_history() {
        let mut s = Session::default();
        let first = s.begin("first", context(), |_, _| Ok(())).unwrap();
        s.complete(first.generation, Ok(reply()));
        let second = s.begin("second", context(), |_, _| Ok(())).unwrap();
        s.complete(second.generation, Err("offline".into()));
        let retry = s.retry(context(), |_, _| Ok(())).unwrap();
        assert_eq!(retry.request.history.len(), 2);
        assert_eq!(retry.request.question, "second");
        assert_eq!(s.messages().len(), 3);
        assert!(s.complete(retry.generation, Ok(reply())));
        assert_eq!(s.messages().len(), 4);
    }
    #[test]
    fn cancel_clear_and_new_request_fence_late_results() {
        let mut s = Session::default();
        let first = s.begin("first", context(), |_, _| Ok(())).unwrap();
        s.cancel();
        assert!(!s.complete(first.generation, Ok(reply())));
        let retry = s.retry(context(), |_, _| Ok(())).unwrap();
        s.clear();
        let new = s.begin("new", context(), |_, _| Ok(())).unwrap();
        assert!(!s.complete(retry.generation, Ok(reply())));
        assert!(s.is_busy());
        assert!(s.complete(new.generation, Ok(reply())));
        assert_eq!(s.messages().len(), 2);
    }
    #[test]
    fn transcript_bound_and_applied_parts_are_per_message() {
        let mut s = Session::default();
        for _ in 0..30 {
            let run = s.begin("question", context(), |_, _| Ok(())).unwrap();
            let mut answer = reply();
            answer.suggestion = Some(Suggestion {
                instant: Some(run.request.context.selected_instant),
                zone_ids: vec!["UTC".into()],
                ..Default::default()
            });
            s.complete(run.generation, Ok(answer));
        }
        assert_eq!(s.messages().len(), TRANSCRIPT_LIMIT);
        let id = s.messages().last().unwrap().id;
        assert!(s.mark_applied(id, Parts::TIME, "time"));
        assert!(!s.messages().last().unwrap().is_fully_applied());
        assert!(s.mark_applied(id, Parts::LOCATIONS, "locations"));
        assert!(s.messages().last().unwrap().is_fully_applied());
        assert!(!s.mark_applied(1, Parts::TIME, "evicted"));
        assert!(s.messages()[1].applied_parts.is_empty());
    }
}
