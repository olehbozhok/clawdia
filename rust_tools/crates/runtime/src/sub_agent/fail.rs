//! `session_fail` — child-side terminal call. Records a `Failed` outcome
//! into the registry and flips the cancel token so the runner exits its
//! loop on the next checkpoint.

use crate::sessions::SessionId;
use crate::sub_agent::outcome::{FailureKind, SubAgentOutcome};
use crate::sub_agent::spawn::SpawnCtx;

#[derive(thiserror::Error, Debug)]
pub enum FailError {
    #[error("unknown session: {0}")]
    Unknown(SessionId),
}

pub async fn session_fail(
    ctx: &SpawnCtx,
    child: &SessionId,
    kind: FailureKind,
    message: String,
    suggested_action: Option<String>,
) -> Result<(), FailError> {
    if ctx.registry.parent_of(child).is_none() {
        return Err(FailError::Unknown(child.clone()));
    }
    // Record outcome BEFORE cancelling: cancel makes the runner return
    // (likely Cancelled), and finalize_child prefers registry.outcome over
    // the runner's return value — so the Failed payload wins the race.
    ctx.registry.record_outcome(
        child,
        SubAgentOutcome::Failed {
            kind,
            message,
            suggested_action,
        },
    );
    ctx.registry.cancel(child);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::SystemMsg;
    use crate::sessions::Principal;
    use crate::sub_agent::spawn::spawn;
    use crate::sub_agent::spawn::test_support::{NeverFinishRunner, ctx_with};
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn session_fail_propagates_structured_payload() {
        let ctx = ctx_with(Arc::new(NeverFinishRunner));
        let parent = ctx
            .sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();
        let child = spawn(&ctx, parent.clone(), "researcher".into(), "x".into())
            .await
            .unwrap();
        session_fail(
            &ctx,
            &child,
            FailureKind::MissingTool {
                name: "fetch_pdf".into(),
            },
            "no tool".into(),
            Some("skip source".into()),
        )
        .await
        .unwrap();

        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let msgs = ctx.inbox.drain(&parent).await.unwrap();
            if let Some(found) = msgs.iter().find_map(|m| match m {
                SystemMsg::SubAgentFinished {
                    outcome:
                        SubAgentOutcome::Failed {
                            kind,
                            message,
                            suggested_action,
                        },
                    ..
                } => Some((kind.clone(), message.clone(), suggested_action.clone())),
                _ => None,
            }) {
                assert!(
                    matches!(found.0, FailureKind::MissingTool { ref name } if name == "fetch_pdf")
                );
                assert_eq!(found.1, "no tool");
                assert_eq!(found.2.as_deref(), Some("skip source"));
                return;
            }
        }
        panic!("expected Failed outcome on parent inbox");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn session_fail_unknown_errors() {
        let ctx = ctx_with(Arc::new(NeverFinishRunner));
        let res = session_fail(
            &ctx,
            &SessionId::for_test("missing"),
            FailureKind::Unknown,
            "x".into(),
            None,
        )
        .await;
        assert!(matches!(res, Err(FailError::Unknown(_))));
    }
}
