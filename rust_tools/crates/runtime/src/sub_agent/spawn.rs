//! Non-blocking sub-agent spawn primitive (design §7).

use crate::inbox::SystemMsg;
use crate::persistence::{Inbox, SessionStore, TerminalOutcome};
use crate::sessions::{Principal, SessionId, Wait};
use crate::sub_agent::outcome::{AbandonReason, SubAgentOutcome};
use crate::sub_agent::registry::SubAgentRegistry;
use async_trait::async_trait;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait ChildRunner: Send + Sync + 'static {
    /// Drive the child's run loop until terminal. Implementations must observe
    /// `cancel` and check `registry.outcome(&child)` between turns to honour
    /// `session_fail` writes.
    async fn run(
        &self,
        child: SessionId,
        prompt: String,
        cancel: CancellationToken,
    ) -> SubAgentOutcome;
}

#[derive(Clone)]
pub struct SpawnCtx {
    pub sessions: Arc<dyn SessionStore>,
    pub inbox: Arc<dyn Inbox>,
    pub registry: SubAgentRegistry,
    pub runner: Arc<dyn ChildRunner>,
    pub principal: Principal,
}

#[derive(thiserror::Error, Debug)]
pub enum SpawnError {
    #[error("parent session not found: {0}")]
    ParentMissing(SessionId),
    #[error("session store error: {0}")]
    Store(#[from] crate::sessions::SessionError),
}

pub async fn spawn(
    ctx: &SpawnCtx,
    parent: SessionId,
    label: String,
    prompt: String,
) -> Result<SessionId, SpawnError> {
    if !ctx.sessions.exists(&parent).await? {
        return Err(SpawnError::ParentMissing(parent));
    }
    let child = ctx
        .sessions
        .create_child(&parent, label.clone(), ctx.principal.clone(), None)
        .await?;
    ctx.sessions
        .add_wait(&parent, Wait::SubAgent(child.clone()))
        .await?;
    let cancel = ctx
        .registry
        .insert(child.clone(), parent.clone(), label.clone());

    let ctx2 = ctx.clone();
    let child2 = child.clone();
    let label2 = label.clone();
    let parent2 = parent.clone();
    tokio::spawn(async move {
        let outcome = ctx2.runner.run(child2.clone(), prompt, cancel.clone()).await;
        // Prefer a recorded outcome (from `session_fail`) over the runner's
        // return value when both exist.
        let final_outcome = ctx2
            .registry
            .outcome(&child2)
            .unwrap_or_else(|| outcome.clone());
        finalize_child(&ctx2, parent2, child2, label2, final_outcome).await;
    });

    Ok(child)
}

pub(crate) async fn finalize_child(
    ctx: &SpawnCtx,
    parent: SessionId,
    child: SessionId,
    label: String,
    outcome: SubAgentOutcome,
) {
    ctx.registry.record_outcome(&child, outcome.clone());

    // Clear any lingering child waits so terminal transition is legal.
    if let Ok(waits) = ctx.sessions.waits(&child).await {
        for w in waits {
            let _ = ctx.sessions.remove_wait(&child, &w).await;
        }
    }

    let terminal = match &outcome {
        SubAgentOutcome::Done { .. } => TerminalOutcome::Done,
        SubAgentOutcome::Failed { .. } => TerminalOutcome::Done,
        SubAgentOutcome::Abandoned { reason } => TerminalOutcome::Abandoned(*reason),
        SubAgentOutcome::Cancelled => TerminalOutcome::Abandoned(AbandonReason::ParentCancel),
    };
    let _ = ctx.sessions.mark_terminal_from_outcome(&child, terminal).await;
    let _ = ctx.inbox.close(&child).await;
    let _ = ctx
        .sessions
        .remove_wait(&parent, &Wait::SubAgent(child.clone()))
        .await;
    let _ = ctx
        .inbox
        .push(
            &parent,
            SystemMsg::SubAgentFinished {
                child_session_id: child,
                agent_label: label,
                outcome,
                finished_at: std::time::Instant::now(),
            },
        )
        .await;
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod test_support {
    use super::*;
    use crate::persistence::memory::{InMemoryInbox, InMemorySessionStore};

    pub fn ctx_with(runner: Arc<dyn ChildRunner>) -> SpawnCtx {
        SpawnCtx {
            sessions: Arc::new(InMemorySessionStore::new()),
            inbox: Arc::new(InMemoryInbox::new()),
            registry: SubAgentRegistry::new(),
            runner,
            principal: Principal("anon".into()),
        }
    }

    pub struct DoneRunner;
    #[async_trait]
    impl ChildRunner for DoneRunner {
        async fn run(
            &self,
            _c: SessionId,
            _p: String,
            _t: CancellationToken,
        ) -> SubAgentOutcome {
            SubAgentOutcome::Done {
                summary: "ok".into(),
                result: "[]".into(),
            }
        }
    }

    pub struct NeverFinishRunner;
    #[async_trait]
    impl ChildRunner for NeverFinishRunner {
        async fn run(
            &self,
            _c: SessionId,
            _p: String,
            cancel: CancellationToken,
        ) -> SubAgentOutcome {
            cancel.cancelled().await;
            SubAgentOutcome::Cancelled
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::sessions::Wait;

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_registers_wait_and_emits_finished() {
        let ctx = ctx_with(Arc::new(DoneRunner));
        let parent = ctx
            .sessions
            .create_root("orchestrator".into(), Principal("anon".into()), None)
            .await
            .unwrap();

        let child = spawn(&ctx, parent.clone(), "researcher".into(), "go".into())
            .await
            .unwrap();

        // Allow the spawned task to run to completion.
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let waits = ctx.sessions.waits(&parent).await.unwrap();
            if !waits.iter().any(|w| matches!(w, Wait::SubAgent(c) if *c == child)) {
                break;
            }
        }

        let msgs = ctx.inbox.drain(&parent).await.unwrap();
        assert!(msgs.iter().any(|m| matches!(
            m,
            SystemMsg::SubAgentFinished {
                outcome: SubAgentOutcome::Done { .. },
                ..
            }
        )));

        let waits_after = ctx.sessions.waits(&parent).await.unwrap();
        assert!(
            !waits_after
                .iter()
                .any(|w| matches!(w, Wait::SubAgent(c) if *c == child))
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_unknown_parent_errors() {
        let ctx = ctx_with(Arc::new(DoneRunner));
        let bogus = SessionId::for_test("nope");
        let res = spawn(&ctx, bogus, "x".into(), "".into()).await;
        assert!(matches!(res, Err(SpawnError::ParentMissing(_))));
    }
}
