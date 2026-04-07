use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Principals ──

/// Principal type derived from agent name in config.
pub type PrincipalType = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    pub principal_type: PrincipalType,
    pub delegation_record_id: Option<Uuid>,
}

// ── Actions (dynamic, derived from tool names) ──

/// An action is simply the tool name as a string.
/// Actions are created dynamically from MCP tool names and sub-agent names.
pub type Action = String;

// ── Authorization request/decision ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    pub principal: Principal,
    pub action: Action,
    pub resource: String,
    pub context: PolicyContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyContext {
    pub delegation_record_id: Option<Uuid>,
    pub requested_domain: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationResult {
    pub decision: AuthorizationDecision,
    pub reason: Option<String>,
}

// ── Audit ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub principal_id: String,
    pub principal_type: PrincipalType,
    pub action: Action,
    pub resource: String,
    pub decision: AuthorizationDecision,
    pub denial_reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl AuditEntry {
    pub fn from_request(req: &AuthorizationRequest, result: &AuthorizationResult) -> Self {
        Self {
            id: Uuid::new_v4(),
            principal_id: req.principal.id.clone(),
            principal_type: req.principal.principal_type.clone(),
            action: req.action.clone(),
            resource: req.resource.clone(),
            decision: result.decision.clone(),
            denial_reason: result.reason.clone(),
            timestamp: Utc::now(),
        }
    }
}
