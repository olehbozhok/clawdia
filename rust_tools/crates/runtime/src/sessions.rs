//! Short, collision-safe sequential session identifiers.
//!
//! Memory note `feedback_unique_ids.md`: LLMs confuse UUIDs. Use short
//! sequential IDs but verify no collision within the generator's lifetime.

use std::sync::atomic::{AtomicU64, Ordering};

/// Opaque session identifier. Always rendered as `s_<base36 counter>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    /// Construct a `SessionId` from a previously-generated string. Validates
    /// the `s_` prefix to defend against accidental misuse (e.g. passing an
    /// arbitrary user string as a session id).
    pub fn from_string(raw: String) -> Result<Self, InvalidSessionId> {
        if !raw.starts_with("s_") || raw.len() < 3 {
            return Err(InvalidSessionId(raw));
        }
        Ok(Self(raw))
    }

    /// Test-only constructor that bypasses prefix validation. Used by Plan 03+
    /// tests for ergonomic short ids. (Decision D16.)
    #[cfg(test)]
    pub fn for_test(raw: &str) -> Self {
        Self(raw.to_string())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid session id: {0:?}")]
pub struct InvalidSessionId(pub String);

/// Sequential `SessionId` generator. Single-process, lock-free.
#[derive(Debug, Default)]
pub struct SessionIdGenerator {
    counter: AtomicU64,
}

impl SessionIdGenerator {
    pub fn new() -> Self {
        Self { counter: AtomicU64::new(0) }
    }

    pub fn next(&self) -> SessionId {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        SessionId(format!("s_{}", to_base36(n)))
    }
}

fn to_base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = Vec::with_capacity(13);
    while n > 0 {
        buf.push(ALPHABET[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).expect("base36 alphabet is ascii")
}

use std::collections::HashSet;
use std::time::Instant;

/// Minimal principal — Plan 04 enriches this with roles/identity transport
/// but the type lives here so sessions can carry it without a circular dep.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Principal(pub String);

// `TicketId` is imported from the canonical home in `crate::approvals::types`
// (Decision D11). Do NOT redefine here.
use crate::approvals::types::TicketId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Sleeping,
    Done,
    Abandoned,
}

impl SessionStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Abandoned)
    }
}

/// External dependency a session is currently blocked on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Wait {
    Approval(TicketId),
    SubAgent(SessionId),
    UserMessage,
}

/// Snapshot of a wait suitable for putting into `SystemMsg::StopWithPendingWaits`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitRef {
    pub kind: WaitRefKind,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitRefKind {
    Approval,
    SubAgent,
    UserMessage,
}

impl From<&Wait> for WaitRef {
    fn from(w: &Wait) -> Self {
        match w {
            Wait::Approval(t) => WaitRef {
                kind: WaitRefKind::Approval,
                label: t.0.clone(),
            },
            Wait::SubAgent(s) => WaitRef {
                kind: WaitRefKind::SubAgent,
                label: s.as_str().to_string(),
            },
            Wait::UserMessage => WaitRef {
                kind: WaitRefKind::UserMessage,
                label: String::new(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub parent_id: Option<SessionId>,
    pub agent_label: String,
    pub principal: Principal,
    pub status: SessionStatus,
    pub deadline: Option<Instant>,
    pub waits: HashSet<Wait>,
    pub started_at: Instant,
    pub last_seen_at: Instant,
}

impl Session {
    pub fn new(
        id: SessionId,
        parent_id: Option<SessionId>,
        agent_label: String,
        principal: Principal,
        deadline: Option<Instant>,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            parent_id,
            agent_label,
            principal,
            status: SessionStatus::Active,
            deadline,
            waits: HashSet::new(),
            started_at: now,
            last_seen_at: now,
        }
    }

    /// Validates and applies a status transition.
    pub fn transition(&mut self, next: SessionStatus) -> Result<(), SessionError> {
        let allowed = matches!(
            (self.status, next),
            (SessionStatus::Active, SessionStatus::Sleeping)
                | (SessionStatus::Active, SessionStatus::Done)
                | (SessionStatus::Active, SessionStatus::Abandoned)
                | (SessionStatus::Sleeping, SessionStatus::Active)
                | (SessionStatus::Sleeping, SessionStatus::Abandoned)
        );
        if !allowed {
            return Err(SessionError::IllegalTransition {
                from: self.status,
                to: next,
            });
        }
        if next == SessionStatus::Done && !self.waits.is_empty() {
            return Err(SessionError::FinishWithPendingWaits);
        }
        if next == SessionStatus::Sleeping && self.waits.is_empty() {
            return Err(SessionError::SleepWithEmptyWaits);
        }
        self.status = next;
        self.last_seen_at = Instant::now();
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("illegal session transition: {from:?} -> {to:?}")]
    IllegalTransition {
        from: SessionStatus,
        to: SessionStatus,
    },
    #[error("cannot finish session: wait set non-empty")]
    FinishWithPendingWaits,
    #[error("cannot sleep session: wait set empty")]
    SleepWithEmptyWaits,
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("inbox closed for session {0}")]
    InboxClosed(String),
}

// End-of-loop evaluator section follows.

use crate::inbox::SystemMsg;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextStatus {
    Done,
    Sleeping,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmStop {
    Stopped,
    MaxTurns,
}

#[derive(Debug, Clone)]
pub struct EndOfLoopInput<'a> {
    pub waits: &'a HashSet<Wait>,
    pub inbox_has_pending: bool,
    pub deadline_passed: bool,
    pub parent_cancelled: bool,
    pub llm_stop: LlmStop,
}

#[derive(Debug, Clone)]
pub struct EndOfLoopDecision {
    pub next: NextStatus,
    pub synthetic_msg: Option<SystemMsg>,
}

pub fn evaluate_end_of_loop(input: EndOfLoopInput<'_>) -> EndOfLoopDecision {
    if input.deadline_passed || input.parent_cancelled {
        return EndOfLoopDecision {
            next: NextStatus::Abandoned,
            synthetic_msg: None,
        };
    }

    if input.waits.is_empty() && !input.inbox_has_pending {
        return EndOfLoopDecision {
            next: NextStatus::Done,
            synthetic_msg: None,
        };
    }

    EndOfLoopDecision {
        next: NextStatus::Sleeping,
        synthetic_msg: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_produces_distinct_sequential_ids() {
        let gen_ = SessionIdGenerator::new();
        let a = gen_.next();
        let b = gen_.next();
        let c = gen_.next();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert!(a.as_str().starts_with("s_"));
    }

    #[test]
    fn generator_no_collisions_under_concurrency() {
        use std::sync::Arc;
        use std::thread;
        let gen_ = Arc::new(SessionIdGenerator::new());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let g = gen_.clone();
            handles.push(thread::spawn(move || {
                (0..1000).map(|_| g.next()).collect::<Vec<_>>()
            }));
        }
        let mut all = Vec::new();
        for h in handles {
            all.extend(h.join().unwrap());
        }
        let unique: std::collections::HashSet<_> = all.iter().cloned().collect();
        assert_eq!(unique.len(), all.len(), "collision detected");
    }

    #[test]
    fn from_string_accepts_well_formed() {
        let id = SessionId::from_string("s_1a".to_string()).unwrap();
        assert_eq!(id.as_str(), "s_1a");
    }

    #[test]
    fn from_string_rejects_missing_prefix() {
        assert!(SessionId::from_string("1a".to_string()).is_err());
    }

    fn mk_session() -> Session {
        Session::new(
            SessionId::from_string("s_1".to_string()).unwrap(),
            None,
            "test".to_string(),
            Principal("anon".to_string()),
            None,
        )
    }

    #[test]
    fn active_to_sleeping_requires_non_empty_waits() {
        let mut s = mk_session();
        assert!(s.transition(SessionStatus::Sleeping).is_err());
        s.waits.insert(Wait::UserMessage);
        s.transition(SessionStatus::Sleeping).unwrap();
        assert_eq!(s.status, SessionStatus::Sleeping);
    }

    #[test]
    fn sleeping_to_active_allowed() {
        let mut s = mk_session();
        s.waits.insert(Wait::UserMessage);
        s.transition(SessionStatus::Sleeping).unwrap();
        s.transition(SessionStatus::Active).unwrap();
        assert_eq!(s.status, SessionStatus::Active);
    }

    #[test]
    fn active_to_done_requires_empty_waits() {
        let mut s = mk_session();
        s.waits.insert(Wait::UserMessage);
        assert!(s.transition(SessionStatus::Done).is_err());
        s.waits.clear();
        s.transition(SessionStatus::Done).unwrap();
        assert_eq!(s.status, SessionStatus::Done);
    }

    #[test]
    fn active_to_abandoned_allowed_with_or_without_waits() {
        let mut s = mk_session();
        s.transition(SessionStatus::Abandoned).unwrap();
        assert_eq!(s.status, SessionStatus::Abandoned);

        let mut s2 = mk_session();
        s2.waits.insert(Wait::UserMessage);
        s2.transition(SessionStatus::Abandoned).unwrap();
    }

    #[test]
    fn terminal_states_are_sticky() {
        let mut s = mk_session();
        s.transition(SessionStatus::Abandoned).unwrap();
        assert!(s.transition(SessionStatus::Active).is_err());
        assert!(s.transition(SessionStatus::Done).is_err());
    }

    #[test]
    fn waitref_from_wait_preserves_label() {
        let w = Wait::SubAgent(SessionId::from_string("s_42".to_string()).unwrap());
        let r: WaitRef = (&w).into();
        assert_eq!(r.kind, WaitRefKind::SubAgent);
        assert_eq!(r.label, "s_42");
    }

    fn empty_waits() -> HashSet<Wait> {
        HashSet::new()
    }

    fn one_wait() -> HashSet<Wait> {
        let mut s = HashSet::new();
        s.insert(Wait::SubAgent(
            SessionId::from_string("s_9".to_string()).unwrap(),
        ));
        s
    }

    #[test]
    fn empty_waits_and_empty_inbox_done() {
        let waits = empty_waits();
        let d = evaluate_end_of_loop(EndOfLoopInput {
            waits: &waits,
            inbox_has_pending: false,
            deadline_passed: false,
            parent_cancelled: false,
            llm_stop: LlmStop::Stopped,
        });
        assert_eq!(d.next, NextStatus::Done);
        assert!(d.synthetic_msg.is_none());
    }

    #[test]
    fn non_empty_waits_sleeps_no_synthetic_msg() {
        let waits = one_wait();
        let d = evaluate_end_of_loop(EndOfLoopInput {
            waits: &waits,
            inbox_has_pending: false,
            deadline_passed: false,
            parent_cancelled: false,
            llm_stop: LlmStop::Stopped,
        });
        assert_eq!(d.next, NextStatus::Sleeping);
        assert!(d.synthetic_msg.is_none());
    }

    #[test]
    fn pending_inbox_with_no_waits_still_sleeps() {
        let waits = empty_waits();
        let d = evaluate_end_of_loop(EndOfLoopInput {
            waits: &waits,
            inbox_has_pending: true,
            deadline_passed: false,
            parent_cancelled: false,
            llm_stop: LlmStop::Stopped,
        });
        assert_eq!(d.next, NextStatus::Sleeping);
        assert!(d.synthetic_msg.is_none());
    }

    #[test]
    fn deadline_overrides_to_abandoned() {
        let waits = one_wait();
        let d = evaluate_end_of_loop(EndOfLoopInput {
            waits: &waits,
            inbox_has_pending: false,
            deadline_passed: true,
            parent_cancelled: false,
            llm_stop: LlmStop::Stopped,
        });
        assert_eq!(d.next, NextStatus::Abandoned);
    }

    #[test]
    fn parent_cancel_overrides_to_abandoned() {
        let waits = empty_waits();
        let d = evaluate_end_of_loop(EndOfLoopInput {
            waits: &waits,
            inbox_has_pending: false,
            deadline_passed: false,
            parent_cancelled: true,
            llm_stop: LlmStop::MaxTurns,
        });
        assert_eq!(d.next, NextStatus::Abandoned);
    }

    #[test]
    fn max_turns_with_waits_still_sleeps_no_warning() {
        let waits = one_wait();
        let d = evaluate_end_of_loop(EndOfLoopInput {
            waits: &waits,
            inbox_has_pending: false,
            deadline_passed: false,
            parent_cancelled: false,
            llm_stop: LlmStop::MaxTurns,
        });
        assert_eq!(d.next, NextStatus::Sleeping);
        assert!(d.synthetic_msg.is_none());
    }
}
