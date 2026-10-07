//! Recording UI policy, independent of native objects and filesystem authority.
//! Logical cancellation never releases a physical worker ticket.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Phase {
    #[default]
    Idle,
    Starting,
    Recording,
    Finalizing,
    Retiring,
    Reviewing,
    Failed,
    Closed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ticket(u64);
#[derive(Debug, Default)]
pub struct Session {
    generation: u64,
    active: Option<Ticket>,
    terminal: Option<bool>,
    phase: Phase,
}
impl Session {
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn busy(&self) -> bool {
        self.active.is_some()
    }
    pub fn begin(&mut self) -> Option<Ticket> {
        if self.busy() || !matches!(self.phase, Phase::Idle | Phase::Failed) {
            return None;
        }
        self.generation += 1;
        let ticket = Ticket(self.generation);
        self.active = Some(ticket);
        self.terminal = None;
        self.phase = Phase::Starting;
        Some(ticket)
    }
    pub fn accepts(&self, ticket: Ticket) -> bool {
        self.active == Some(ticket)
            && ticket.0 == self.generation
            && !matches!(self.phase, Phase::Closed | Phase::Retiring)
            && self.terminal.is_none()
    }
    pub fn started(&mut self, ticket: Ticket) -> bool {
        if !self.accepts(ticket) {
            return false;
        }
        if self.phase == Phase::Starting {
            self.phase = Phase::Recording;
        }
        true
    }
    /// Stop during startup is queued once; a late Started cannot undo it.
    pub fn stop(&mut self) -> Option<Ticket> {
        if !matches!(self.phase, Phase::Starting | Phase::Recording) {
            return None;
        }
        self.phase = Phase::Finalizing;
        self.active
    }
    pub fn completed(&mut self, ticket: Ticket, success: bool) -> bool {
        if !self.accepts(ticket) {
            return false;
        }
        self.terminal = Some(success);
        self.phase = Phase::Finalizing;
        true
    }
    /// Only the owner may report physical drain, after all callbacks and cleanup.
    pub fn drained(&mut self, ticket: Ticket) -> bool {
        if self.active != Some(ticket) {
            return false;
        }
        self.active = None;
        if self.phase == Phase::Closed {
            return true;
        }
        self.phase = if ticket.0 != self.generation {
            Phase::Idle
        } else if self.terminal == Some(true) {
            Phase::Reviewing
        } else {
            Phase::Failed
        };
        self.terminal = None;
        true
    }
    pub fn cancel(&mut self) {
        if self.phase == Phase::Closed {
            return;
        }
        self.generation += 1;
        self.terminal = None;
        self.phase = if self.busy() {
            Phase::Retiring
        } else {
            Phase::Idle
        };
    }
    pub fn close(&mut self) {
        self.cancel();
        self.phase = Phase::Closed;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn double_clicks_and_stop_during_start_are_once_only() {
        let mut s = Session::default();
        let t = s.begin().unwrap();
        assert!(s.begin().is_none());
        assert_eq!(s.stop(), Some(t));
        assert!(s.stop().is_none());
        assert!(s.started(t));
        assert_eq!(s.phase(), Phase::Finalizing);
        assert!(s.completed(t, true));
        assert!(!s.started(t));
        assert!(!s.completed(t, false));
        assert!(s.busy());
        assert!(s.begin().is_none());
        assert!(s.drained(t));
        assert_eq!(s.phase(), Phase::Reviewing);
    }
    #[test]
    fn cancelled_generation_cannot_revive_and_stays_busy_until_drain() {
        let mut s = Session::default();
        let old = s.begin().unwrap();
        s.cancel();
        assert_eq!(s.phase(), Phase::Retiring);
        assert!(!s.completed(old, true));
        assert!(!s.started(old));
        assert!(s.begin().is_none());
        assert!(!s.drained(Ticket(99)));
        assert!(s.busy());
        assert!(s.drained(old));
        let new = s.begin().unwrap();
        assert_ne!(new, old);
        assert!(!s.completed(old, true));
        assert!(!s.drained(old));
        assert!(s.completed(new, false));
        assert!(s.drained(new));
        assert_eq!(s.phase(), Phase::Failed);
        assert!(s.begin().is_some());
    }
    #[test]
    fn close_cannot_reopen_from_late_success_or_drain() {
        let mut s = Session::default();
        let t = s.begin().unwrap();
        s.close();
        assert!(!s.completed(t, true));
        assert!(s.busy());
        s.drained(t);
        assert_eq!(s.phase(), Phase::Closed);
        assert!(s.begin().is_none());
    }
    #[test]
    fn terminal_success_waits_for_physical_retirement_and_cancel_can_hide_review() {
        let mut s = Session::default();
        let t = s.begin().unwrap();
        s.completed(t, true);
        assert_eq!(s.phase(), Phase::Finalizing);
        s.cancel();
        s.drained(t);
        assert_eq!(s.phase(), Phase::Idle);
    }
}
