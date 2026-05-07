//! Cascade test (design §7) — A spawns B spawns C; C sleeps on a non-sub-agent
//! wait, A and B remain Sleeping. Resolving C wakes the chain via
//! `SubAgentFinished` events, with no special graph-traversal code.

#![cfg(test)]

use crate::config::RuntimeConfig;
use crate::inbox::SystemMsg;
use crate::sessions::{Principal, SessionId, Wait};
use crate::sub_agent::outcome::SubAgentOutcome;
use crate::sub_agent::registry::SubAgentRegistry;
use crate::sub_agent::spawn::{ChildRunner, SpawnCtx, spawn};
use async_trait::async_trait;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Runner that blocks until cancelled, then returns whatever outcome the
/// registry holds (set via `finish_child_for_test`) or `Cancelled` if none.
struct RegistryWatcherRunner {
    registry: SubAgentRegistry,
}

#[async_trait]
impl ChildRunner for RegistryWatcherRunner {
    async fn run(
        &self,
        child: SessionId,
        _prompt: String,
        cancel: CancellationToken,
    ) -> SubAgentOutcome {
        cancel.cancelled().await;
        self.registry
            .outcome(&child)
            .unwrap_or(SubAgentOutcome::Cancelled)
    }
}

fn ctx_for_cascade() -> SpawnCtx {
    use crate::persistence::memory::{InMemoryInbox, InMemorySessionStore};
    let registry = SubAgentRegistry::new();
    SpawnCtx {
        sessions: Arc::new(InMemorySessionStore::new()),
        inbox: Arc::new(InMemoryInbox::new()),
        registry: registry.clone(),
        runner: Arc::new(RegistryWatcherRunner { registry }),
        principal: Principal("anon".into()),
        runtime_config: RuntimeConfig::default(),
    }
}

async fn finish_child_for_test(ctx: &SpawnCtx, child: &SessionId, outcome: SubAgentOutcome) {
    ctx.registry.record_outcome(child, outcome);
    ctx.registry.cancel(child);
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_chain_sleeps_until_leaf_resolves() {
    let ctx = ctx_for_cascade();
    let a = ctx
        .sessions
        .create_root("A".into(), Principal("anon".into()), None)
        .await
        .unwrap();
    let b = spawn(&ctx, a.clone(), "B".into(), "".into()).await.unwrap();
    let c = spawn(&ctx, b.clone(), "C".into(), "".into()).await.unwrap();

    // C blocks on a UserMessage wait (synthetic).
    ctx.sessions.add_wait(&c, Wait::UserMessage).await.unwrap();

    // No SubAgentFinished delivered yet (no child terminated).
    assert!(ctx.inbox.drain(&a).await.unwrap().is_empty());
    assert!(ctx.inbox.drain(&b).await.unwrap().is_empty());

    // Resolve C: leaf finishes Done.
    ctx.sessions
        .remove_wait(&c, &Wait::UserMessage)
        .await
        .unwrap();
    finish_child_for_test(
        &ctx,
        &c,
        SubAgentOutcome::Done {
            summary: "leaf".into(),
            result: "{}".into(),
        },
    )
    .await;

    // B got SubAgentFinished from C.
    let mut got_b = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let msgs = ctx.inbox.drain(&b).await.unwrap();
        if msgs
            .iter()
            .any(|m| matches!(m, SystemMsg::SubAgentFinished { .. }))
        {
            got_b = true;
            break;
        }
    }
    assert!(got_b, "B did not receive SubAgentFinished from C");

    // Now finish B; A should observe.
    finish_child_for_test(
        &ctx,
        &b,
        SubAgentOutcome::Done {
            summary: "mid".into(),
            result: "{}".into(),
        },
    )
    .await;

    let mut got_a = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let msgs = ctx.inbox.drain(&a).await.unwrap();
        if msgs
            .iter()
            .any(|m| matches!(m, SystemMsg::SubAgentFinished { .. }))
        {
            got_a = true;
            break;
        }
    }
    assert!(got_a, "A did not receive SubAgentFinished from B");
}
