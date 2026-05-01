//! Approval outcome + approver identity (Decision D2).
//! Plan 04 wires gateway/signing around these enums but does NOT redefine them.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApprovalOutcome {
    Approved,
    Denied { reason: String },
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApproverKind {
    LocalKey { key_id: String },
    Jwt { sub: String, iss: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApproverIdentity {
    pub kind: ApproverKind,
    pub roles: Vec<String>,
}
