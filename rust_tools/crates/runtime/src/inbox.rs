//! Inbox message types and rendering.
//!
//! `SystemMsg` is the strongly-typed sum of every event the runtime can push
//! into a session's inbox. Rendering to LLM context is a *function of the
//! variant*, never a stored field — see design §3.3.

use crate::approvals::outcome::{ApprovalOutcome, ApproverIdentity};
use crate::approvals::types::TicketId;
use crate::sessions::{Session, SessionId, WaitRef, WaitRefKind};
use crate::sub_agent::outcome::SubAgentOutcome;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum SystemMsg {
    ApprovalDecided {
        ticket_id: TicketId,
        action_kind: String,
        decision: ApprovalOutcome,
        approver: ApproverIdentity,
        decided_at: Instant,
    },
    SubAgentFinished {
        child_session_id: SessionId,
        agent_label: String,
        outcome: SubAgentOutcome,
        finished_at: Instant,
    },
    UserMessage {
        text: String,
        received_at: Instant,
    },
    DeadlineWarning {
        remaining: Duration,
    },
}

/// Render an inbox message to a human-readable line for the LLM.
pub fn render(msg: &SystemMsg) -> String {
    match msg {
        SystemMsg::ApprovalDecided {
            ticket_id,
            action_kind,
            ..
        } => format!(
            "approval ticket {} for action {} decided",
            ticket_id.0, action_kind
        ),
        SystemMsg::SubAgentFinished {
            child_session_id,
            agent_label,
            ..
        } => format!(
            "sub-agent {}/{} finished",
            agent_label,
            child_session_id.as_str()
        ),
        SystemMsg::UserMessage { text, .. } => {
            let preview: String = text.chars().take(80).collect();
            format!("user said: {preview}")
        }
        SystemMsg::DeadlineWarning { remaining } => {
            format!("deadline warning: {} seconds remaining", remaining.as_secs())
        }
    }
}

/// Render the wait-state header + drained inbox events as a single concatenated
/// system message (D4).
pub fn render_for_turn(session: &Session, drained: &[SystemMsg]) -> String {
    let mut out = String::new();
    if !session.waits.is_empty() {
        let parts: Vec<String> = session
            .waits
            .iter()
            .map(WaitRef::from)
            .map(|w| match w.kind {
                WaitRefKind::Approval => format!("approval:{}", w.label),
                WaitRefKind::SubAgent => format!("sub_agent:{}", w.label),
                WaitRefKind::UserMessage => "user_message".to_string(),
            })
            .collect();
        out.push_str(&format!("Currently waiting on: [{}]\n", parts.join(", ")));
    }
    for msg in drained {
        out.push_str("- ");
        out.push_str(&render(msg));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::SessionId;

    #[test]
    fn renders_approval_decided() {
        use crate::approvals::outcome::{ApproverIdentity, ApproverKind};
        let msg = SystemMsg::ApprovalDecided {
            ticket_id: TicketId("tk_1".to_string()),
            action_kind: "doc.publish".to_string(),
            decision: ApprovalOutcome::Approved,
            approver: ApproverIdentity {
                kind: ApproverKind::LocalKey { key_id: "k1".to_string() },
                roles: vec!["operator".to_string()],
            },
            decided_at: Instant::now(),
        };
        let s = render(&msg);
        assert!(s.contains("tk_1"));
        assert!(s.contains("doc.publish"));
    }

    #[test]
    fn renders_subagent_finished() {
        let msg = SystemMsg::SubAgentFinished {
            child_session_id: SessionId::from_string("s_7".to_string()).unwrap(),
            agent_label: "researcher".to_string(),
            outcome: SubAgentOutcome::Done {
                summary: "ok".to_string(),
                result: "{}".to_string(),
            },
            finished_at: Instant::now(),
        };
        let s = render(&msg);
        assert!(s.contains("researcher"));
        assert!(s.contains("s_7"));
    }

    #[test]
    fn renders_user_message_with_unicode_truncation() {
        let msg = SystemMsg::UserMessage {
            text: "🌊".repeat(200),
            received_at: Instant::now(),
        };
        let s = render(&msg);
        assert!(s.starts_with("user said: "));
    }

    #[test]
    fn renders_deadline_warning() {
        let msg = SystemMsg::DeadlineWarning {
            remaining: Duration::from_secs(42),
        };
        let s = render(&msg);
        assert!(s.contains("42"));
    }

    #[test]
    fn render_for_turn_includes_wait_header_when_waits_present() {
        use crate::sessions::{Principal, Session, SessionId, Wait};
        let mut sess = Session::new(
            SessionId::from_string("s_1".to_string()).unwrap(),
            None,
            "t".to_string(),
            Principal("anon".to_string()),
            None,
        );
        sess.waits.insert(Wait::Approval(TicketId("tk_9".to_string())));
        let out = render_for_turn(&sess, &[]);
        assert!(out.contains("Currently waiting on:"));
        assert!(out.contains("approval:tk_9"));
    }
}
