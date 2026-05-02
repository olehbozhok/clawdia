//! Registry of approval-gated action handlers. `approval_execute` resolves
//! `action_kind` to a registered handler, then dispatches the call with the
//! args stored on the ticket — never with attacker-controlled re-supplied
//! args. The registry itself does NOT run the runtime gate; it is invoked
//! by `approval_execute` only AFTER `ApprovalGateway::gate_call` succeeds
//! (six-step verification + atomic consume).

use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ActionRegistryError {
    #[error("no handler registered for action_kind {0:?}")]
    Unknown(String),
    #[error("handler error: {0}")]
    Handler(String),
}

#[async_trait]
pub trait ActionHandler: Send + Sync {
    async fn execute(&self, args: &Value) -> Result<Value, ActionRegistryError>;
}

#[async_trait]
pub trait ActionRegistry: Send + Sync {
    async fn execute(
        &self,
        action_kind: &str,
        args: &Value,
    ) -> Result<Value, ActionRegistryError>;
}

#[derive(Default)]
pub struct InMemoryActionRegistry {
    handlers: DashMap<String, Arc<dyn ActionHandler>>,
}

impl InMemoryActionRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn register(&self, action_kind: impl Into<String>, handler: Arc<dyn ActionHandler>) {
        self.handlers.insert(action_kind.into(), handler);
    }
}

#[async_trait]
impl ActionRegistry for InMemoryActionRegistry {
    async fn execute(
        &self,
        action_kind: &str,
        args: &Value,
    ) -> Result<Value, ActionRegistryError> {
        let handler = self
            .handlers
            .get(action_kind)
            .ok_or_else(|| ActionRegistryError::Unknown(action_kind.to_string()))?
            .clone();
        handler.execute(args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    struct RecordingHandler {
        calls: Arc<Mutex<Vec<Value>>>,
    }

    #[async_trait]
    impl ActionHandler for RecordingHandler {
        async fn execute(&self, args: &Value) -> Result<Value, ActionRegistryError> {
            self.calls.lock().unwrap().push(args.clone());
            Ok(json!({"ok": true}))
        }
    }

    #[tokio::test]
    async fn registry_dispatches_to_registered_handler() {
        let reg = InMemoryActionRegistry::default();
        let calls = Arc::new(Mutex::new(vec![]));
        reg.register(
            "doc.publish_live",
            Arc::new(RecordingHandler {
                calls: calls.clone(),
            }),
        );
        let out = reg
            .execute("doc.publish_live", &json!({"campaign_id": "c1"}))
            .await
            .unwrap();
        assert_eq!(out, json!({"ok": true}));
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn registry_unknown_action_returns_error() {
        let reg = InMemoryActionRegistry::default();
        let err = reg.execute("nope", &json!({})).await.unwrap_err();
        assert!(matches!(err, ActionRegistryError::Unknown(_)));
    }
}
