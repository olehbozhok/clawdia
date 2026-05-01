//! `agent_get` inspector — non-blocking lookup of a sub-agent's state.

use crate::sessions::{SessionId, SessionStatus};
use crate::sub_agent::outcome::{AbandonReason, SubAgentOutcome};
use crate::sub_agent::spawn::SpawnCtx;
use crate::sub_agent::state::AgentState;

#[derive(thiserror::Error, Debug)]
pub enum InspectError {
    #[error("unknown session: {0}")]
    Unknown(SessionId),
    #[error("session store error: {0}")]
    Store(#[from] crate::sessions::SessionError),
}

pub async fn agent_get(ctx: &SpawnCtx, child: &SessionId) -> Result<AgentState, InspectError> {
    if !ctx.sessions.exists(child).await? {
        return Err(InspectError::Unknown(child.clone()));
    }
    // Registry first: finalize_child records outcome before flipping session
    // to terminal, so during that window registry is the fresher source.
    if let Some(outcome) = ctx.registry.outcome(child) {
        return Ok(match outcome {
            SubAgentOutcome::Done { summary, result } => AgentState::Done { summary, result },
            SubAgentOutcome::Failed {
                kind,
                message,
                suggested_action,
            } => AgentState::Failed {
                kind,
                message,
                suggested_action,
            },
            SubAgentOutcome::Abandoned { reason } => AgentState::Abandoned { reason },
            SubAgentOutcome::Cancelled => AgentState::Cancelled,
        });
    }
    let status = ctx
        .sessions
        .status(child)
        .await?
        .ok_or_else(|| InspectError::Unknown(child.clone()))?;
    let waits = ctx.sessions.wait_refs(child).await?;
    Ok(match status {
        SessionStatus::Active => AgentState::Running,
        SessionStatus::Sleeping => AgentState::Sleeping { waits },
        SessionStatus::Done => AgentState::Done {
            summary: String::new(),
            result: String::new(),
        },
        SessionStatus::Abandoned => AgentState::Abandoned {
            reason: AbandonReason::Ttl,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::Principal;
    use crate::sub_agent::spawn::spawn;
    use crate::sub_agent::spawn::test_support::{DoneRunner, NeverFinishRunner, ctx_with};
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn unknown_child_is_inspect_error() {
        let ctx = ctx_with(Arc::new(DoneRunner));
        let bogus = SessionId::for_test("nope");
        assert!(matches!(
            agent_get(&ctx, &bogus).await,
            Err(InspectError::Unknown(_))
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn running_child_returns_running_or_sleeping() {
        let ctx = ctx_with(Arc::new(NeverFinishRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let child = spawn(&ctx, parent, "r".into(), "x".into()).await.unwrap();
        let st = agent_get(&ctx, &child).await.unwrap();
        assert!(matches!(
            st,
            AgentState::Running | AgentState::Sleeping { .. }
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn done_child_returns_done() {
        let ctx = ctx_with(Arc::new(DoneRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let child = spawn(&ctx, parent, "r".into(), "x".into()).await.unwrap();
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            if matches!(
                agent_get(&ctx, &child).await.unwrap(),
                AgentState::Done { .. }
            ) {
                return;
            }
        }
        panic!("child never reached Done");
    }
}
