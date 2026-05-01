//! `agent_cancel` — flips the cancellation token. The runner observes it and
//! returns `SubAgentOutcome::Cancelled`; `finalize_child` does the rest.

use crate::sessions::SessionId;
use crate::sub_agent::spawn::SpawnCtx;

#[derive(thiserror::Error, Debug)]
pub enum CancelError {
    #[error("unknown session: {0}")]
    Unknown(SessionId),
}

/// Request cancellation of a sub-agent by flipping its registry cancel token.
/// Cooperative: the runner must observe the token between turns. Final
/// transition (status, parent notification) happens in `finalize_child` once
/// the runner exits. Returns `Unknown` if the child is not in the registry
/// (never spawned, or already finalized and reaped).
pub async fn agent_cancel(ctx: &SpawnCtx, child: &SessionId) -> Result<(), CancelError> {
    if !ctx.registry.cancel(child) {
        return Err(CancelError::Unknown(child.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::SystemMsg;
    use crate::sessions::Principal;
    use crate::sub_agent::inspect::agent_get;
    use crate::sub_agent::outcome::SubAgentOutcome;
    use crate::sub_agent::spawn::spawn;
    use crate::sub_agent::spawn::test_support::{NeverFinishRunner, ctx_with};
    use crate::sub_agent::state::AgentState;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_long_running_child_emits_cancelled() {
        let ctx = ctx_with(Arc::new(NeverFinishRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let child = spawn(&ctx, parent.clone(), "r".into(), "x".into())
            .await
            .unwrap();
        agent_cancel(&ctx, &child).await.unwrap();
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            if matches!(
                agent_get(&ctx, &child).await.unwrap(),
                AgentState::Cancelled
            ) {
                break;
            }
        }
        assert!(matches!(
            agent_get(&ctx, &child).await.unwrap(),
            AgentState::Cancelled
        ));
        let msgs = ctx.inbox.drain(&parent).await.unwrap();
        assert!(msgs.iter().any(|m| matches!(
            m,
            SystemMsg::SubAgentFinished {
                outcome: SubAgentOutcome::Cancelled,
                ..
            }
        )));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_unknown_returns_unknown_error() {
        let ctx = ctx_with(Arc::new(NeverFinishRunner));
        assert!(matches!(
            agent_cancel(&ctx, &SessionId::for_test("missing")).await,
            Err(CancelError::Unknown(_))
        ));
    }
}
