//! `AgentState` — inspector return type for `agent_get` (design §9). Distinct
//! from `SubAgentOutcome` because it covers live states (`Running`,
//! `Sleeping { waits }`) in addition to terminal ones.

use crate::sessions::WaitRef;
use crate::sub_agent::outcome::{AbandonReason, FailureKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum AgentState {
    Running,
    Sleeping {
        waits: Vec<WaitRef>,
    },
    Done {
        summary: String,
        result: String,
    },
    Failed {
        kind: FailureKind,
        message: String,
        suggested_action: Option<String>,
    },
    Abandoned {
        reason: AbandonReason,
    },
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_state_running_serializes() {
        let s = AgentState::Running;
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("Running"));
    }

    #[test]
    fn agent_state_failed_round_trips_kind() {
        let s = AgentState::Failed {
            kind: FailureKind::MissingTool {
                name: "fetch_pdf".into(),
            },
            message: "x".into(),
            suggested_action: None,
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("MissingTool"));
        assert!(j.contains("fetch_pdf"));
    }
}
