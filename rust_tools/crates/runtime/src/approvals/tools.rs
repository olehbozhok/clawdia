//! Agent-facing MCP tools for the approval lifecycle.
//!
//! Each tool struct carries the calling-session id at construction time so an
//! agent cannot impersonate another session by passing a forged id.
//!
//! Coverage v1: `approval_request`, `approval_status`, `approval_describe`,
//! `approval_list_mine`. `approval_execute` (atomic gate+dispatch) is deferred
//! until an ActionRegistry exists in the runtime — agents currently invoke the
//! gated tool directly after Cedar reads `context.approval`.

use crate::approvals::gateway::{ApprovalGateway, GatewayError};
use crate::approvals::registry::{ActionRegistry, ActionRegistryError};
use crate::approvals::types::{ApprovalRequest, TicketId, TicketStatus};
use crate::sessions::SessionId;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApprovalToolError {
    #[error("gateway: {0}")]
    Gateway(#[from] GatewayError),
    #[error("registry: {0}")]
    Registry(#[from] ActionRegistryError),
    #[error("ticket not found")]
    NotFound,
}

fn status_str(s: TicketStatus) -> &'static str {
    match s {
        TicketStatus::Pending => "pending",
        TicketStatus::Approved => "approved",
        TicketStatus::Denied => "denied",
        TicketStatus::Expired => "expired",
        TicketStatus::Orphaned => "orphaned",
        TicketStatus::Consumed => "consumed",
    }
}

const DEFAULT_TTL_SECS: u64 = 600;

// ── approval_request ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRequestArgs {
    pub action_kind: String,
    pub args: serde_json::Value,
    pub reason: String,
    #[serde(default)]
    pub hint: Option<String>,
    /// Override TTL for this ticket. Capped by session deadline upstream.
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalRequestResult {
    pub ticket_id: String,
    pub status: String,
    pub args_hash: String,
}

pub struct ApprovalRequestTool {
    pub gateway: Arc<ApprovalGateway>,
    pub caller_session_id: SessionId,
}

impl Tool for ApprovalRequestTool {
    const NAME: &'static str = "approval_request";
    type Error = ApprovalToolError;
    type Args = ApprovalRequestArgs;
    type Output = ApprovalRequestResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Request human approval for a gated action. Idempotent on (session, action_kind, \
                 args): a second call with identical inputs returns the existing ticket. Floats \
                 in args are rejected — use integers."
                    .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ApprovalRequestArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let ttl = Duration::from_secs(args.ttl_seconds.unwrap_or(DEFAULT_TTL_SECS));
        let ticket = self
            .gateway
            .request(ApprovalRequest {
                action_kind: args.action_kind,
                args: args.args,
                reason: args.reason,
                hint: args.hint,
                requester: self.caller_session_id.clone(),
                ttl,
                metadata: serde_json::Value::Null,
            })
            .await?;
        Ok(ApprovalRequestResult {
            ticket_id: ticket.id.0,
            status: status_str(ticket.status).to_string(),
            args_hash: ticket.args_hash,
        })
    }
}

// ── approval_status ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalStatusArgs {
    pub ticket_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalStatusResult {
    pub ticket_id: String,
    pub status: String,
}

pub struct ApprovalStatusTool {
    pub gateway: Arc<ApprovalGateway>,
}

impl Tool for ApprovalStatusTool {
    const NAME: &'static str = "approval_status";
    type Error = ApprovalToolError;
    type Args = ApprovalStatusArgs;
    type Output = ApprovalStatusResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Look up the lifecycle status of an approval ticket.".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ApprovalStatusArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let id = TicketId(args.ticket_id);
        let ticket = self
            .gateway
            .ticket_store()
            .get(&id)
            .map_err(GatewayError::Store)?
            .ok_or(ApprovalToolError::NotFound)?;
        Ok(ApprovalStatusResult {
            ticket_id: ticket.id.0,
            status: status_str(ticket.status).to_string(),
        })
    }
}

// ── approval_describe ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalDescribeArgs {
    pub ticket_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalDescribeResult {
    pub ticket_id: String,
    pub session_id: String,
    pub action_kind: String,
    pub args: serde_json::Value,
    pub args_hash: String,
    pub reason: String,
    pub hint: Option<String>,
    pub status: String,
    pub denial_reason: Option<String>,
}

pub struct ApprovalDescribeTool {
    pub gateway: Arc<ApprovalGateway>,
}

impl Tool for ApprovalDescribeTool {
    const NAME: &'static str = "approval_describe";
    type Error = ApprovalToolError;
    type Args = ApprovalDescribeArgs;
    type Output = ApprovalDescribeResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Return the public view of an approval ticket (no signature material)."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ApprovalDescribeArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let id = TicketId(args.ticket_id);
        let ticket = self
            .gateway
            .ticket_store()
            .get(&id)
            .map_err(GatewayError::Store)?
            .ok_or(ApprovalToolError::NotFound)?;
        let denial_reason = ticket
            .decision
            .as_ref()
            .and_then(|d| d.reason.clone());
        Ok(ApprovalDescribeResult {
            ticket_id: ticket.id.0,
            session_id: ticket.session_id.into_string(),
            action_kind: ticket.action_kind,
            args: ticket.args,
            args_hash: ticket.args_hash,
            reason: ticket.reason,
            hint: ticket.hint,
            status: status_str(ticket.status).to_string(),
            denial_reason,
        })
    }
}

// ── approval_execute ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalExecuteArgs {
    pub ticket_id: String,
    /// Args supplied at execute time. MUST hash identically to the args
    /// stored on the ticket — drift returns `args_drift`. The handler is
    /// invoked with the args from the ticket (single source of truth),
    /// not these — these only feed the integrity check.
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalExecuteResult {
    pub ticket_id: String,
    pub action_kind: String,
    pub result: serde_json::Value,
}

pub struct ApprovalExecuteTool {
    pub gateway: Arc<ApprovalGateway>,
    pub registry: Arc<dyn ActionRegistry>,
    pub caller_session_id: SessionId,
}

impl Tool for ApprovalExecuteTool {
    const NAME: &'static str = "approval_execute";
    type Error = ApprovalToolError;
    type Args = ApprovalExecuteArgs;
    type Output = ApprovalExecuteResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Atomically gate and execute an approved action. Runs the six-step runtime gate \
                 (existence, status, args_hash, expiry, signature, atomic consume) and then \
                 dispatches via the action registry using the args stored on the ticket. Calling \
                 a gated action directly will be denied by Cedar; callers must use this tool."
                    .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ApprovalExecuteArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let id = TicketId(args.ticket_id);
        let ok = self
            .gateway
            .gate_call(&id, &self.caller_session_id, &args.args)
            .await?;
        let result = self
            .registry
            .execute(&ok.ticket.action_kind, &ok.ticket.args)
            .await?;
        Ok(ApprovalExecuteResult {
            ticket_id: ok.ticket.id.0,
            action_kind: ok.ticket.action_kind,
            result,
        })
    }
}

// ── approval_list_mine ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalListMineArgs {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalListMineEntry {
    pub ticket_id: String,
    pub action_kind: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalListMineResult {
    pub tickets: Vec<ApprovalListMineEntry>,
}

pub struct ApprovalListMineTool {
    pub gateway: Arc<ApprovalGateway>,
    pub caller_session_id: SessionId,
}

impl Tool for ApprovalListMineTool {
    const NAME: &'static str = "approval_list_mine";
    type Error = ApprovalToolError;
    type Args = ApprovalListMineArgs;
    type Output = ApprovalListMineResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List approval tickets owned by the calling session.".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ApprovalListMineArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let listed = self
            .gateway
            .ticket_store()
            .list_by_session(&self.caller_session_id)
            .map_err(GatewayError::Store)?;
        Ok(ApprovalListMineResult {
            tickets: listed
                .into_iter()
                .map(|t| ApprovalListMineEntry {
                    ticket_id: t.id.0,
                    action_kind: t.action_kind,
                    status: status_str(t.status).to_string(),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::outcome::{ApprovalOutcome, ApproverIdentity, ApproverKind};
    use crate::approvals::registry::{ActionHandler, InMemoryActionRegistry};
    use async_trait::async_trait;
    use crate::persistence::Inbox;
    use crate::persistence::SessionStore;
    use crate::persistence::memory::{InMemoryInbox, InMemorySessionStore};
    use crate::persistence::tickets::{InMemoryTicketStore, TicketStore};
    use crate::sessions::Principal;
    use serde_json::json;

    fn approver() -> ApproverIdentity {
        ApproverIdentity {
            kind: ApproverKind::LocalKey {
                key_id: "k1".into(),
            },
            roles: vec!["campaign_owner".into()],
        }
    }

    async fn harness() -> (Arc<ApprovalGateway>, SessionId, SessionId) {
        let tickets: Arc<dyn TicketStore> = InMemoryTicketStore::new();
        let sessions: Arc<dyn SessionStore> = InMemorySessionStore::arc();
        let inbox: Arc<dyn Inbox> = InMemoryInbox::arc();
        let sid = sessions
            .create_root("orchestrator".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let other = sessions
            .create_root("orchestrator".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let gw = Arc::new(ApprovalGateway::new(
            tickets,
            sessions,
            inbox,
            vec![0xABu8; 32],
        ));
        (gw, sid, other)
    }

    #[tokio::test]
    async fn approval_request_returns_pending_ticket() {
        let (gw, sid, _) = harness().await;
        let tool = ApprovalRequestTool {
            gateway: gw,
            caller_session_id: sid,
        };
        let res = tool
            .call(ApprovalRequestArgs {
                action_kind: "doc.publish_live".into(),
                args: json!({"campaign_id": "c1"}),
                reason: "go live".into(),
                hint: None,
                ttl_seconds: Some(60),
            })
            .await
            .unwrap();
        assert_eq!(res.status, "pending");
        assert!(res.args_hash.starts_with("sha256:"));
    }

    #[tokio::test]
    async fn approval_request_rejects_floats_in_args() {
        let (gw, sid, _) = harness().await;
        let tool = ApprovalRequestTool {
            gateway: gw,
            caller_session_id: sid,
        };
        let err = tool
            .call(ApprovalRequestArgs {
                action_kind: "act".into(),
                args: json!({"x": 1.5}),
                reason: "r".into(),
                hint: None,
                ttl_seconds: None,
            })
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("floats"));
    }

    #[tokio::test]
    async fn approval_status_returns_pending_then_approved() {
        let (gw, sid, _) = harness().await;
        let req = ApprovalRequestTool {
            gateway: gw.clone(),
            caller_session_id: sid.clone(),
        };
        let res = req
            .call(ApprovalRequestArgs {
                action_kind: "act".into(),
                args: json!({"x": 1}),
                reason: "r".into(),
                hint: None,
                ttl_seconds: Some(60),
            })
            .await
            .unwrap();
        let status_tool = ApprovalStatusTool {
            gateway: gw.clone(),
        };
        let s1 = status_tool
            .call(ApprovalStatusArgs {
                ticket_id: res.ticket_id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(s1.status, "pending");

        gw.decide_local(
            &TicketId(res.ticket_id.clone()),
            ApprovalOutcome::Approved,
            approver(),
        )
        .await
        .unwrap();
        let s2 = status_tool
            .call(ApprovalStatusArgs {
                ticket_id: res.ticket_id,
            })
            .await
            .unwrap();
        assert_eq!(s2.status, "approved");
    }

    #[tokio::test]
    async fn approval_describe_returns_public_view() {
        let (gw, sid, _) = harness().await;
        let req = ApprovalRequestTool {
            gateway: gw.clone(),
            caller_session_id: sid.clone(),
        };
        let res = req
            .call(ApprovalRequestArgs {
                action_kind: "act".into(),
                args: json!({"x": 1}),
                reason: "why".into(),
                hint: Some("hint".into()),
                ttl_seconds: Some(60),
            })
            .await
            .unwrap();
        let describe = ApprovalDescribeTool { gateway: gw };
        let d = describe
            .call(ApprovalDescribeArgs {
                ticket_id: res.ticket_id,
            })
            .await
            .unwrap();
        assert_eq!(d.action_kind, "act");
        assert_eq!(d.reason, "why");
        assert_eq!(d.hint.as_deref(), Some("hint"));
        assert_eq!(d.session_id, sid.into_string());
    }

    struct RecordingHandler {
        calls: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    }

    #[async_trait]
    impl ActionHandler for RecordingHandler {
        async fn execute(
            &self,
            args: &serde_json::Value,
        ) -> Result<serde_json::Value, crate::approvals::registry::ActionRegistryError> {
            self.calls.lock().unwrap().push(args.clone());
            Ok(json!({"published": true}))
        }
    }

    async fn approved_ticket(
        gw: &Arc<ApprovalGateway>,
        sid: &SessionId,
        action_kind: &str,
        args: serde_json::Value,
    ) -> String {
        let req = ApprovalRequestTool {
            gateway: gw.clone(),
            caller_session_id: sid.clone(),
        };
        let res = req
            .call(ApprovalRequestArgs {
                action_kind: action_kind.into(),
                args,
                reason: "r".into(),
                hint: None,
                ttl_seconds: Some(60),
            })
            .await
            .unwrap();
        gw.decide_local(
            &TicketId(res.ticket_id.clone()),
            ApprovalOutcome::Approved,
            approver(),
        )
        .await
        .unwrap();
        res.ticket_id
    }

    #[tokio::test]
    async fn approval_execute_dispatches_with_stored_args() {
        let (gw, sid, _) = harness().await;
        let reg = InMemoryActionRegistry::default();
        let calls = Arc::new(std::sync::Mutex::new(vec![]));
        reg.register(
            "doc.publish_live",
            Arc::new(RecordingHandler {
                calls: calls.clone(),
            }),
        );
        let reg: Arc<dyn ActionRegistry> = Arc::new(reg);
        let stored = json!({"campaign_id": "c1"});
        let tid = approved_ticket(&gw, &sid, "doc.publish_live", stored.clone()).await;
        let exec = ApprovalExecuteTool {
            gateway: gw,
            registry: reg,
            caller_session_id: sid,
        };
        let out = exec
            .call(ApprovalExecuteArgs {
                ticket_id: tid.clone(),
                args: stored.clone(),
            })
            .await
            .unwrap();
        assert_eq!(out.ticket_id, tid);
        assert_eq!(out.result, json!({"published": true}));
        let recorded = calls.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0], stored);
    }

    #[tokio::test]
    async fn approval_execute_second_call_returns_consumed() {
        let (gw, sid, _) = harness().await;
        let reg: Arc<dyn ActionRegistry> = {
            let r = InMemoryActionRegistry::default();
            r.register(
                "act",
                Arc::new(RecordingHandler {
                    calls: Arc::new(std::sync::Mutex::new(vec![])),
                }),
            );
            Arc::new(r)
        };
        let args = json!({"x": 1});
        let tid = approved_ticket(&gw, &sid, "act", args.clone()).await;
        let exec = ApprovalExecuteTool {
            gateway: gw,
            registry: reg,
            caller_session_id: sid,
        };
        exec.call(ApprovalExecuteArgs {
            ticket_id: tid.clone(),
            args: args.clone(),
        })
        .await
        .unwrap();
        let err = exec
            .call(ApprovalExecuteArgs {
                ticket_id: tid,
                args,
            })
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("consumed"));
    }

    #[tokio::test]
    async fn approval_execute_session_mismatch_rejected() {
        let (gw, sid, other) = harness().await;
        let reg: Arc<dyn ActionRegistry> = {
            let r = InMemoryActionRegistry::default();
            r.register(
                "act",
                Arc::new(RecordingHandler {
                    calls: Arc::new(std::sync::Mutex::new(vec![])),
                }),
            );
            Arc::new(r)
        };
        let args = json!({"x": 1});
        let tid = approved_ticket(&gw, &sid, "act", args.clone()).await;
        let exec = ApprovalExecuteTool {
            gateway: gw,
            registry: reg,
            caller_session_id: other,
        };
        let err = exec
            .call(ApprovalExecuteArgs {
                ticket_id: tid,
                args,
            })
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("session mismatch"));
    }

    #[tokio::test]
    async fn approval_execute_args_drift_rejected() {
        let (gw, sid, _) = harness().await;
        let reg: Arc<dyn ActionRegistry> = {
            let r = InMemoryActionRegistry::default();
            r.register(
                "act",
                Arc::new(RecordingHandler {
                    calls: Arc::new(std::sync::Mutex::new(vec![])),
                }),
            );
            Arc::new(r)
        };
        let tid = approved_ticket(&gw, &sid, "act", json!({"x": 1})).await;
        let exec = ApprovalExecuteTool {
            gateway: gw,
            registry: reg,
            caller_session_id: sid,
        };
        let err = exec
            .call(ApprovalExecuteArgs {
                ticket_id: tid,
                args: json!({"x": 2}),
            })
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("args drift"));
    }

    #[tokio::test]
    async fn approval_list_mine_filters_by_caller_session() {
        let (gw, sid, other) = harness().await;
        ApprovalRequestTool {
            gateway: gw.clone(),
            caller_session_id: sid.clone(),
        }
        .call(ApprovalRequestArgs {
            action_kind: "a1".into(),
            args: json!({"x": 1}),
            reason: "r".into(),
            hint: None,
            ttl_seconds: None,
        })
        .await
        .unwrap();
        ApprovalRequestTool {
            gateway: gw.clone(),
            caller_session_id: other,
        }
        .call(ApprovalRequestArgs {
            action_kind: "a2".into(),
            args: json!({"x": 2}),
            reason: "r".into(),
            hint: None,
            ttl_seconds: None,
        })
        .await
        .unwrap();

        let list_tool = ApprovalListMineTool {
            gateway: gw,
            caller_session_id: sid,
        };
        let res = list_tool.call(ApprovalListMineArgs {}).await.unwrap();
        assert_eq!(res.tickets.len(), 1);
        assert_eq!(res.tickets[0].action_kind, "a1");
    }
}
