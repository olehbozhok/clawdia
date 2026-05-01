//! Live registry of running sub-agents (design §7).

use crate::sessions::SessionId;
use crate::sub_agent::outcome::SubAgentOutcome;
use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct ChildHandle {
    pub parent: SessionId,
    pub label: String,
    pub cancel: CancellationToken,
    pub outcome: Arc<OnceCell<SubAgentOutcome>>,
}

#[derive(Default, Clone)]
pub struct SubAgentRegistry {
    inner: Arc<DashMap<SessionId, ChildHandle>>,
}

impl SubAgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, child: SessionId, parent: SessionId, label: String) -> CancellationToken {
        let cancel = CancellationToken::new();
        self.inner.insert(
            child,
            ChildHandle {
                parent,
                label,
                cancel: cancel.clone(),
                outcome: Arc::new(OnceCell::new()),
            },
        );
        cancel
    }

    pub fn contains(&self, child: &SessionId) -> bool {
        self.inner.contains_key(child)
    }

    pub fn cancel(&self, child: &SessionId) -> bool {
        match self.inner.get(child) {
            Some(h) => {
                h.cancel.cancel();
                true
            }
            None => false,
        }
    }

    pub fn record_outcome(&self, child: &SessionId, outcome: SubAgentOutcome) {
        if let Some(h) = self.inner.get(child) {
            let _ = h.outcome.set(outcome);
        }
    }

    pub fn outcome(&self, child: &SessionId) -> Option<SubAgentOutcome> {
        self.inner.get(child).and_then(|h| h.outcome.get().cloned())
    }

    pub fn parent_of(&self, child: &SessionId) -> Option<SessionId> {
        self.inner.get(child).map(|h| h.parent.clone())
    }

    pub fn label_of(&self, child: &SessionId) -> Option<String> {
        self.inner.get(child).map(|h| h.label.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_then_get_returns_handle() {
        let reg = SubAgentRegistry::new();
        let sid = SessionId::for_test("c1");
        let parent = SessionId::for_test("p1");
        let token = reg.insert(sid.clone(), parent, "researcher".into());
        assert!(reg.contains(&sid));
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_flips_token() {
        let reg = SubAgentRegistry::new();
        let sid = SessionId::for_test("c2");
        let parent = SessionId::for_test("p2");
        let token = reg.insert(sid.clone(), parent, "verifier".into());
        assert!(reg.cancel(&sid));
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_unknown_returns_false() {
        let reg = SubAgentRegistry::new();
        assert!(!reg.cancel(&SessionId::for_test("missing")));
    }
}
