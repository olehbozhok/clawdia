//! `notify_human` MCP tool. Per design §17 — fire-and-forget signal from agent
//! to human. Cedar gates the action (orchestrator-only by default); this tool
//! is the agent-facing entry point that wraps `emit_notification`.
//!
//! ## Wiring status
//!
//! Same shape as the `approval_*` tools in `approvals/tools.rs`: the struct +
//! `impl Tool` are complete, but `build_agent` / `build_orchestrator` in
//! `agents.rs` does not yet attach built-in runtime tools via `.tool(...)`.
//! Plan 06 (`RuntimeGlue`) constructs the per-run `NotificationStore`,
//! `NotificationIdGenerator`, and root `SessionId` and is the wiring step.
//! Until then, this tool is exercised only by unit tests.

use crate::notifications::emit::{EmitArgs, NotifyError, emit_notification};
use crate::notifications::types::{NotificationIdGenerator, NotificationRefs, Severity};
use crate::persistence::notifications::NotificationStore;
use crate::sessions::SessionId;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Hard cap on body size — protects log pipeline + future web rendering.
/// Design is silent on a value; 64 KiB is generous for any human-readable
/// alert and well below tracing's per-event practical limit.
pub const BODY_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum NotifyHumanError {
    #[error("subject must not be empty")]
    EmptySubject,
    #[error("body exceeds {BODY_MAX_BYTES} bytes")]
    BodyTooLarge,
    #[error("emit: {0}")]
    Emit(#[from] NotifyError),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NotifyHumanArgs {
    pub severity: Severity,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub refs: NotificationRefs,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotifyHumanResult {
    pub notification_id: String,
}

pub struct NotifyHumanTool {
    pub store: Arc<dyn NotificationStore>,
    pub idgen: Arc<NotificationIdGenerator>,
    pub caller_session_id: SessionId,
}

impl Tool for NotifyHumanTool {
    const NAME: &'static str = "notify_human";
    type Error = NotifyHumanError;
    type Args = NotifyHumanArgs;
    type Output = NotifyHumanResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Notify the human (fire-and-forget). Does not block, does not create a Wait. Use \
                 for status updates and blocker alerts where action is needed before the agent \
                 can proceed. Severity: info | warn | error | blocker."
                    .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(NotifyHumanArgs))
                .expect("schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if args.subject.trim().is_empty() {
            return Err(NotifyHumanError::EmptySubject);
        }
        if args.body.len() > BODY_MAX_BYTES {
            return Err(NotifyHumanError::BodyTooLarge);
        }
        let id = emit_notification(
            &*self.store,
            &self.idgen,
            EmitArgs {
                severity: args.severity,
                subject: args.subject,
                body: args.body,
                refs: args.refs,
                emitted_by: self.caller_session_id.clone(),
            },
        )?;
        Ok(NotifyHumanResult {
            notification_id: id.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::notifications::InMemoryNotificationStore;

    fn harness() -> (NotifyHumanTool, Arc<InMemoryNotificationStore>) {
        let store = InMemoryNotificationStore::new();
        let tool = NotifyHumanTool {
            store: store.clone(),
            idgen: Arc::new(NotificationIdGenerator::new()),
            caller_session_id: SessionId::for_test("s_orchestrator"),
        };
        (tool, store)
    }

    fn args(severity: Severity, subject: &str, body: &str) -> NotifyHumanArgs {
        NotifyHumanArgs {
            severity,
            subject: subject.into(),
            body: body.into(),
            refs: NotificationRefs::default(),
        }
    }

    #[tokio::test]
    async fn notify_human_records_and_returns_id() {
        let (tool, store) = harness();
        let res = tool
            .call(args(Severity::Warn, "subj", "body"))
            .await
            .unwrap();
        let listed = store.list(None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id.0, res.notification_id);
        assert_eq!(listed[0].severity, Severity::Warn);
    }

    #[tokio::test]
    async fn notify_human_rejects_empty_subject() {
        let (tool, store) = harness();
        let err = tool
            .call(args(Severity::Info, "", "body"))
            .await
            .unwrap_err();
        assert!(matches!(err, NotifyHumanError::EmptySubject));
        assert!(store.list(None).unwrap().is_empty());
    }

    #[tokio::test]
    async fn notify_human_rejects_whitespace_only_subject() {
        let (tool, _) = harness();
        let err = tool
            .call(args(Severity::Info, "   \t\n", "body"))
            .await
            .unwrap_err();
        assert!(matches!(err, NotifyHumanError::EmptySubject));
    }

    #[tokio::test]
    async fn notify_human_rejects_oversize_body() {
        let (tool, store) = harness();
        let big = "a".repeat(BODY_MAX_BYTES + 1);
        let err = tool
            .call(args(Severity::Info, "s", &big))
            .await
            .unwrap_err();
        assert!(matches!(err, NotifyHumanError::BodyTooLarge));
        assert!(store.list(None).unwrap().is_empty());
    }

    #[tokio::test]
    async fn notify_human_invalid_severity_rejected_at_deserialize() {
        // The MCP layer will deserialize args from JSON. Test that invalid
        // severity strings fail to parse (serde rejects unknown variants).
        let raw = r#"{"severity":"critical","subject":"s","body":"b"}"#;
        let parsed: Result<NotifyHumanArgs, _> = serde_json::from_str(raw);
        assert!(parsed.is_err(), "unknown severity must not deserialize");
    }

    #[tokio::test]
    async fn notify_human_attaches_caller_session_id() {
        let (tool, store) = harness();
        tool.call(args(Severity::Info, "s", "b")).await.unwrap();
        let n = store.list(None).unwrap().pop().unwrap();
        assert_eq!(n.emitted_by, SessionId::for_test("s_orchestrator"));
    }
}
