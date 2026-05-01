//! Runtime-side rig tools for sub-agent control: `agent_spawn`, `agent_get`,
//! `agent_cancel`, `session_fail`, `session_done` (design §9, §21, plan D14).
//!
//! Each tool carries the calling-session id at construction time — never read
//! from LLM arguments — so an agent cannot impersonate another session.

use crate::inbox::SystemMsg;
use crate::sessions::{SessionId, SessionStatus};
use crate::sub_agent::cancel::agent_cancel;
use crate::sub_agent::fail::session_fail;
use crate::sub_agent::inspect::agent_get;
use crate::sub_agent::outcome::{FailureKind, SubAgentOutcome};
use crate::sub_agent::spawn::{SpawnCtx, finalize_child, spawn};
use crate::sub_agent::state::AgentState;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(thiserror::Error, Debug)]
pub enum SubAgentToolError {
    #[error("spawn: {0}")]
    Spawn(#[from] crate::sub_agent::spawn::SpawnError),
    #[error("inspect: {0}")]
    Inspect(#[from] crate::sub_agent::inspect::InspectError),
    #[error("cancel: {0}")]
    Cancel(#[from] crate::sub_agent::cancel::CancelError),
    #[error("fail: {0}")]
    Fail(#[from] crate::sub_agent::fail::FailError),
    #[error("session: {0}")]
    Session(#[from] crate::sessions::SessionError),
    #[error("invalid session id: {0}")]
    InvalidSessionId(String),
}

fn parse_sid(raw: &str) -> Result<SessionId, SubAgentToolError> {
    SessionId::from_string(raw.to_string())
        .map_err(|e| SubAgentToolError::InvalidSessionId(format!("{e:?}")))
}

// ── agent_spawn ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentSpawnArgs {
    pub label: String,
    pub prompt: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentSpawnResult {
    pub child_session_id: String,
    pub status: String,
}

pub struct AgentSpawnTool {
    pub ctx: Arc<SpawnCtx>,
    pub caller_session_id: SessionId,
}

impl Tool for AgentSpawnTool {
    const NAME: &'static str = "agent_spawn";
    type Error = SubAgentToolError;
    type Args = AgentSpawnArgs;
    type Output = AgentSpawnResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Spawn a sub-agent. Returns immediately with the child session id; the parent is \
                 notified via SubAgentFinished when the child terminates. The child runs under \
                 the same principal as the caller."
                    .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(AgentSpawnArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let child = spawn(
            self.ctx.as_ref(),
            self.caller_session_id.clone(),
            args.label,
            args.prompt,
        )
        .await?;
        Ok(AgentSpawnResult {
            child_session_id: child.into_string(),
            status: "running".into(),
        })
    }
}

// ── agent_get ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentGetArgs {
    pub child_session_id: String,
}

pub struct AgentGetTool {
    pub ctx: Arc<SpawnCtx>,
}

impl Tool for AgentGetTool {
    const NAME: &'static str = "agent_get";
    type Error = SubAgentToolError;
    type Args = AgentGetArgs;
    type Output = AgentState;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Inspect a sub-agent's state. Non-blocking; never errors on lifecycle."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(AgentGetArgs)).expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let sid = parse_sid(&args.child_session_id)?;
        Ok(agent_get(self.ctx.as_ref(), &sid).await?)
    }
}

// ── agent_cancel ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentCancelArgs {
    pub child_session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmptyResult {}

pub struct AgentCancelTool {
    pub ctx: Arc<SpawnCtx>,
}

impl Tool for AgentCancelTool {
    const NAME: &'static str = "agent_cancel";
    type Error = SubAgentToolError;
    type Args = AgentCancelArgs;
    type Output = EmptyResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Cancel a sub-agent. The child runner observes the cancel and terminates \
                          with Cancelled outcome."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(AgentCancelArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let sid = parse_sid(&args.child_session_id)?;
        agent_cancel(self.ctx.as_ref(), &sid).await?;
        Ok(EmptyResult {})
    }
}

// ── session_fail ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionFailArgs {
    pub kind: FailureKind,
    pub message: String,
    #[serde(default)]
    pub suggested_action: Option<String>,
}

pub struct SessionFailTool {
    pub ctx: Arc<SpawnCtx>,
    pub caller_session_id: SessionId,
}

impl Tool for SessionFailTool {
    const NAME: &'static str = "session_fail";
    type Error = SubAgentToolError;
    type Args = SessionFailArgs;
    type Output = EmptyResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Terminate the calling session with a structured Failed outcome. The \
                          parent is notified with the kind/message/suggested_action payload."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(SessionFailArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        session_fail(
            self.ctx.as_ref(),
            &self.caller_session_id,
            args.kind,
            args.message,
            args.suggested_action,
        )
        .await?;
        Ok(EmptyResult {})
    }
}

// ── session_done ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionDoneArgs {
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionDoneStatus {
    Done,
    RefusedPendingWaits,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDoneResult {
    pub status: SessionDoneStatus,
}

pub struct SessionDoneTool {
    pub ctx: Arc<SpawnCtx>,
    pub caller_session_id: SessionId,
}

impl Tool for SessionDoneTool {
    const NAME: &'static str = "session_done";
    type Error = SubAgentToolError;
    type Args = SessionDoneArgs;
    type Output = SessionDoneResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Mark the calling session Done with a one-line summary. Refuses (with a \
                          system message back to your inbox) if you still have pending waits."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(SessionDoneArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let sid = &self.caller_session_id;
        let session = self
            .ctx
            .sessions
            .get(sid)
            .await?
            .ok_or_else(|| SubAgentToolError::InvalidSessionId(sid.as_str().to_string()))?;

        // Refuse via inbox rather than erroring: gives the agent a chance to
        // observe its own pending waits, drain them, and retry. Terminating
        // here would also bypass child-side wait cleanup that finalize_child
        // does on the spawn-driven path.
        if !session.waits.is_empty() {
            let n = session.waits.len();
            self.ctx
                .inbox
                .push(
                    sid,
                    SystemMsg::UserMessage {
                        text: format!(
                            "session_done refused: still waiting on {n} item(s) — call again \
                             after wait set drains"
                        ),
                        received_at: std::time::Instant::now(),
                    },
                )
                .await?;
            return Ok(SessionDoneResult {
                status: SessionDoneStatus::RefusedPendingWaits,
            });
        }

        // Empty waits: terminate.
        if let Some(parent) = session.parent_id.clone() {
            // Child path: route through finalize_child so registry + parent
            // notification stay consistent with spawn-driven termination.
            finalize_child(
                self.ctx.as_ref(),
                parent,
                sid.clone(),
                session.agent_label.clone(),
                SubAgentOutcome::Done {
                    summary: args.summary,
                    result: String::new(),
                },
            )
            .await;
        } else {
            // Root path: just transition.
            self.ctx
                .sessions
                .set_status(sid, SessionStatus::Done)
                .await?;
            self.ctx.inbox.close(sid).await?;
        }
        Ok(SessionDoneResult {
            status: SessionDoneStatus::Done,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{Principal, Wait};
    use crate::sub_agent::spawn::test_support::{DoneRunner, NeverFinishRunner, ctx_with};

    fn arc_ctx(runner: Arc<dyn crate::sub_agent::spawn::ChildRunner>) -> Arc<SpawnCtx> {
        Arc::new(ctx_with(runner))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_spawn_tool_returns_child_id_and_marks_running() {
        let ctx = arc_ctx(Arc::new(NeverFinishRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let tool = AgentSpawnTool {
            ctx: ctx.clone(),
            caller_session_id: parent.clone(),
        };
        let resp = tool
            .call(AgentSpawnArgs {
                label: "researcher".into(),
                prompt: "find sources".into(),
            })
            .await
            .unwrap();
        assert_eq!(resp.status, "running");
        let cid = SessionId::from_string(resp.child_session_id).unwrap();
        let waits = ctx.sessions.waits(&parent).await.unwrap();
        assert!(
            waits
                .iter()
                .any(|w| matches!(w, Wait::SubAgent(c) if *c == cid))
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_get_tool_returns_running_for_live_child() {
        let ctx = arc_ctx(Arc::new(NeverFinishRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let spawn_tool = AgentSpawnTool {
            ctx: ctx.clone(),
            caller_session_id: parent.clone(),
        };
        let resp = spawn_tool
            .call(AgentSpawnArgs {
                label: "r".into(),
                prompt: "x".into(),
            })
            .await
            .unwrap();
        let get_tool = AgentGetTool { ctx: ctx.clone() };
        let st = get_tool
            .call(AgentGetArgs {
                child_session_id: resp.child_session_id,
            })
            .await
            .unwrap();
        assert!(matches!(
            st,
            AgentState::Running | AgentState::Sleeping { .. }
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_cancel_tool_transitions_to_cancelled() {
        let ctx = arc_ctx(Arc::new(NeverFinishRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let spawn_tool = AgentSpawnTool {
            ctx: ctx.clone(),
            caller_session_id: parent,
        };
        let resp = spawn_tool
            .call(AgentSpawnArgs {
                label: "r".into(),
                prompt: "x".into(),
            })
            .await
            .unwrap();
        let cancel_tool = AgentCancelTool { ctx: ctx.clone() };
        cancel_tool
            .call(AgentCancelArgs {
                child_session_id: resp.child_session_id.clone(),
            })
            .await
            .unwrap();
        let get_tool = AgentGetTool { ctx };
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let st = get_tool
                .call(AgentGetArgs {
                    child_session_id: resp.child_session_id.clone(),
                })
                .await
                .unwrap();
            if matches!(st, AgentState::Cancelled) {
                return;
            }
        }
        panic!("child never reached Cancelled");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn session_fail_tool_records_failed_outcome() {
        let ctx = arc_ctx(Arc::new(NeverFinishRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let spawn_tool = AgentSpawnTool {
            ctx: ctx.clone(),
            caller_session_id: parent.clone(),
        };
        let resp = spawn_tool
            .call(AgentSpawnArgs {
                label: "r".into(),
                prompt: "x".into(),
            })
            .await
            .unwrap();
        let cid = SessionId::from_string(resp.child_session_id).unwrap();
        let fail_tool = SessionFailTool {
            ctx: ctx.clone(),
            caller_session_id: cid.clone(),
        };
        fail_tool
            .call(SessionFailArgs {
                kind: FailureKind::MissingTool {
                    name: "fetch_pdf".into(),
                },
                message: "no tool".into(),
                suggested_action: None,
            })
            .await
            .unwrap();
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let msgs = ctx.inbox.drain(&parent).await.unwrap();
            if msgs.iter().any(|m| {
                matches!(
                    m,
                    SystemMsg::SubAgentFinished {
                        outcome: SubAgentOutcome::Failed { .. },
                        ..
                    }
                )
            }) {
                return;
            }
        }
        panic!("expected Failed in parent inbox");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn session_done_tool_fails_to_inbox_when_waits_present() {
        let ctx = arc_ctx(Arc::new(DoneRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        ctx.sessions
            .add_wait(&parent, Wait::UserMessage)
            .await
            .unwrap();
        let tool = SessionDoneTool {
            ctx: ctx.clone(),
            caller_session_id: parent.clone(),
        };
        let res = tool
            .call(SessionDoneArgs {
                summary: "x".into(),
            })
            .await
            .unwrap();
        assert!(matches!(res.status, SessionDoneStatus::RefusedPendingWaits));
        let drained = ctx.inbox.drain(&parent).await.unwrap();
        assert!(
            drained
                .iter()
                .any(|m| matches!(m, SystemMsg::UserMessage { .. })),
            "expected refusal system msg"
        );
        assert_ne!(
            ctx.sessions.status(&parent).await.unwrap(),
            Some(SessionStatus::Done)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn session_done_tool_transitions_done_when_waits_empty() {
        let ctx = arc_ctx(Arc::new(DoneRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let tool = SessionDoneTool {
            ctx: ctx.clone(),
            caller_session_id: parent.clone(),
        };
        let res = tool
            .call(SessionDoneArgs {
                summary: "all good".into(),
            })
            .await
            .unwrap();
        assert!(matches!(res.status, SessionDoneStatus::Done));
        assert_eq!(
            ctx.sessions.status(&parent).await.unwrap(),
            Some(SessionStatus::Done)
        );
    }
}
