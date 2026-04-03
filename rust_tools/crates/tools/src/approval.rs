use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Who performed the action — an agent or a human approver.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Actor {
    Agent { id: String, role: String },
    Human { sub: String, role: String },
}

/// A single approval or rejection decision on a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub id: Uuid,
    pub document_id: String,
    pub actor: Actor,
    pub decision: Decision,
    pub reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approved,
    Rejected,
    NeedsRevision,
}

/// Tracks the full approval lifecycle of a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentApprovalState {
    pub document_id: String,
    pub records: Vec<ApprovalRecord>,
    pub current_status: Decision,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DocumentApprovalState {
    pub fn new(document_id: String) -> Self {
        let now = Utc::now();
        Self {
            document_id,
            records: Vec::new(),
            current_status: Decision::NeedsRevision,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_record(&mut self, record: ApprovalRecord) {
        self.current_status = record.decision;
        self.updated_at = record.timestamp;
        self.records.push(record);
    }
}
