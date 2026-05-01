//! Sub-agent outcome enum (Decision D2). Plan 03 expands the surrounding
//! sub_agent/ folder with registry, spawn, inspect, cancel, fail, render,
//! mcp_tools — but does NOT redefine these enums.

use crate::approvals::types::TicketId;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SubAgentOutcome {
    Done { summary: String, result: String },
    Failed {
        kind: FailureKind,
        message: String,
        suggested_action: Option<String>,
    },
    Abandoned { reason: AbandonReason },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FailureKind {
    MissingTool { name: String },
    InsufficientPermission { action: String },
    AmbiguousRequest { question: String },
    ExternalError { source: String },
    HumanInputRequired { ticket_id: TicketId },
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AbandonReason {
    Ttl,
    ParentCancel,
    DenialTerminal,
}
