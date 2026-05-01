//! Canonical approval ticket id and supporting ticket types.
//!
//! `TicketId` was seeded by Plan 02. Plan 04 expands the module with the
//! lifecycle types (`TicketStatus`, `Decision`, `Ticket`, `ApprovalRequest`).

use crate::approvals::outcome::ApproverIdentity;
use crate::sessions::SessionId;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct TicketId(pub String);

impl std::fmt::Display for TicketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TicketStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Orphaned,
    Consumed,
}

impl TicketStatus {
    /// Terminal-and-not-actionable. `Approved` is terminal-but-actionable
    /// until consumed, so it is intentionally not "terminal" here.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Denied | Self::Expired | Self::Orphaned | Self::Consumed
        )
    }
    pub fn is_actionable(self) -> bool {
        matches!(self, Self::Approved)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub approver: ApproverIdentity,
    pub signature: Vec<u8>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub id: TicketId,
    pub session_id: SessionId,
    pub action_kind: String,
    pub args: serde_json::Value,
    pub args_hash: String,
    pub reason: String,
    pub hint: Option<String>,
    pub status: TicketStatus,
    pub decision: Option<Decision>,
    #[serde(skip, default = "Instant::now")]
    pub created_at: Instant,
    #[serde(skip, default = "Instant::now")]
    pub expires_at: Instant,
    #[serde(skip)]
    pub decided_at: Option<Instant>,
    #[serde(skip)]
    pub consumed_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub action_kind: String,
    pub args: serde_json::Value,
    pub reason: String,
    pub hint: Option<String>,
    pub requester: SessionId,
    pub ttl: Duration,
    pub metadata: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::outcome::{ApproverIdentity, ApproverKind};

    #[test]
    fn status_terminal_classification() {
        assert!(!TicketStatus::Pending.is_terminal());
        assert!(!TicketStatus::Approved.is_terminal());
        assert!(TicketStatus::Denied.is_terminal());
        assert!(TicketStatus::Expired.is_terminal());
        assert!(TicketStatus::Orphaned.is_terminal());
        assert!(TicketStatus::Consumed.is_terminal());

        assert!(TicketStatus::Approved.is_actionable());
        assert!(!TicketStatus::Pending.is_actionable());
        assert!(!TicketStatus::Denied.is_actionable());
    }

    #[test]
    fn ticket_serde_roundtrip() {
        let now = Instant::now();
        let ticket = Ticket {
            id: TicketId("tk_abc".into()),
            session_id: SessionId::for_test("s_1"),
            action_kind: "doc.publish_live".into(),
            args: serde_json::json!({"campaign_id": "c1"}),
            args_hash: "sha256:deadbeef".into(),
            reason: "publish".into(),
            hint: Some("hint".into()),
            status: TicketStatus::Pending,
            decision: Some(Decision {
                approver: ApproverIdentity {
                    kind: ApproverKind::LocalKey {
                        key_id: "k1".into(),
                    },
                    roles: vec!["campaign_owner".into()],
                },
                signature: vec![1, 2, 3, 4],
                reason: None,
            }),
            created_at: now,
            expires_at: now,
            decided_at: None,
            consumed_at: None,
        };
        let json = serde_json::to_string(&ticket).unwrap();
        let back: Ticket = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, ticket.id);
        assert_eq!(back.action_kind, ticket.action_kind);
        assert_eq!(back.args_hash, ticket.args_hash);
        assert_eq!(back.status, ticket.status);
        assert!(back.decision.is_some());
    }
}
