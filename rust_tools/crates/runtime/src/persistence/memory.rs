//! In-memory implementations of `SessionStore` and `Inbox`.
//!
//! Single-process v1. Sessions do not survive runtime restarts.

use super::{Inbox, Result, SessionEvent, SessionStore, TerminalOutcome};
use crate::inbox::SystemMsg;
use crate::sessions::{Principal, Session, SessionError, SessionId, SessionStatus, Wait, WaitRef};
use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, broadcast};

/// Per-session event broadcast lane.
struct SessionLane {
    events: tokio::sync::broadcast::Sender<SessionEvent>,
}

impl SessionLane {
    fn new() -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        Self { events: tx }
    }
}

#[derive(Default)]
pub struct InMemorySessionStore {
    sessions: DashMap<SessionId, Session>,
    lanes: DashMap<SessionId, std::sync::Arc<SessionLane>>,
    ids: crate::sessions::SessionIdGenerator,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn arc() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::new())
    }

    fn lane(&self, id: &SessionId) -> std::sync::Arc<SessionLane> {
        self.lanes
            .entry(id.clone())
            .or_insert_with(|| std::sync::Arc::new(SessionLane::new()))
            .clone()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn create_root(
        &self,
        agent_label: String,
        principal: Principal,
        deadline: Option<Instant>,
    ) -> Result<SessionId> {
        let id = self.ids.next();
        let s = Session::new(id.clone(), None, agent_label, principal, deadline);
        self.sessions.insert(id.clone(), s);
        Ok(id)
    }

    async fn create_child(
        &self,
        parent: &SessionId,
        agent_label: String,
        principal: Principal,
        deadline: Option<Instant>,
    ) -> Result<SessionId> {
        let id = self.ids.next();
        let s = Session::new(id.clone(), Some(parent.clone()), agent_label, principal, deadline);
        self.sessions.insert(id.clone(), s);
        Ok(id)
    }

    async fn exists(&self, id: &SessionId) -> Result<bool> {
        Ok(self.sessions.contains_key(id))
    }

    async fn get(&self, id: &SessionId) -> Result<Option<Session>> {
        Ok(self.sessions.get(id).map(|r| r.clone()))
    }

    async fn status(&self, id: &SessionId) -> Result<Option<SessionStatus>> {
        Ok(self.sessions.get(id).map(|r| r.status))
    }

    async fn set_status(&self, id: &SessionId, status: SessionStatus) -> Result<()> {
        let mut entry = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| SessionError::NotFound(id.as_str().to_string()))?;
        entry.transition(status)?;
        let _ = self.lane(id).events.send(SessionEvent::StatusChanged(status));
        Ok(())
    }

    async fn add_wait(&self, id: &SessionId, wait: Wait) -> Result<()> {
        let mut entry = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| SessionError::NotFound(id.as_str().to_string()))?;
        entry.waits.insert(wait);
        let _ = self.lane(id).events.send(SessionEvent::WaitAdded);
        Ok(())
    }

    async fn remove_wait(&self, id: &SessionId, wait: &Wait) -> Result<()> {
        let mut entry = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| SessionError::NotFound(id.as_str().to_string()))?;
        entry.waits.remove(wait);
        let _ = self.lane(id).events.send(SessionEvent::WaitResolved);
        Ok(())
    }

    async fn waits(&self, id: &SessionId) -> Result<HashSet<Wait>> {
        Ok(self
            .sessions
            .get(id)
            .map(|r| r.waits.clone())
            .unwrap_or_default())
    }

    async fn wait_refs(&self, id: &SessionId) -> Result<Vec<WaitRef>> {
        Ok(self
            .sessions
            .get(id)
            .map(|r| r.waits.iter().map(WaitRef::from).collect())
            .unwrap_or_default())
    }

    async fn list_active(&self) -> Result<Vec<Session>> {
        Ok(self
            .sessions
            .iter()
            .filter(|r| !r.status.is_terminal())
            .map(|r| r.clone())
            .collect())
    }

    async fn mark_terminal_from_outcome(
        &self,
        id: &SessionId,
        outcome: TerminalOutcome,
    ) -> Result<()> {
        let next = match outcome {
            TerminalOutcome::Done => SessionStatus::Done,
            TerminalOutcome::Abandoned(_) => SessionStatus::Abandoned,
        };
        self.set_status(id, next).await
    }

    fn subscribe(&self, id: &SessionId) -> tokio::sync::broadcast::Receiver<SessionEvent> {
        self.lane(id).events.subscribe()
    }
}

struct InboxLane {
    queue: Mutex<VecDeque<SystemMsg>>,
    wake: broadcast::Sender<()>,
}

impl InboxLane {
    fn new() -> Self {
        let (tx, _rx) = broadcast::channel(64);
        Self {
            queue: Mutex::new(VecDeque::new()),
            wake: tx,
        }
    }
}

#[derive(Default)]
pub struct InMemoryInbox {
    lanes: DashMap<SessionId, Arc<InboxLane>>,
}

impl InMemoryInbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn arc() -> Arc<Self> {
        Arc::new(Self::new())
    }

    fn lane(&self, id: &SessionId) -> Arc<InboxLane> {
        self.lanes
            .entry(id.clone())
            .or_insert_with(|| Arc::new(InboxLane::new()))
            .clone()
    }
}

#[async_trait]
impl Inbox for InMemoryInbox {
    async fn push(&self, session: &SessionId, msg: SystemMsg) -> Result<()> {
        let lane = self.lane(session);
        let mut q = lane.queue.lock().await;
        q.push_back(msg);
        let _ = lane.wake.send(());
        Ok(())
    }

    async fn drain(&self, session: &SessionId) -> Result<Vec<SystemMsg>> {
        let lane = self.lane(session);
        let mut q = lane.queue.lock().await;
        Ok(q.drain(..).collect())
    }

    fn subscribe(&self, session: &SessionId) -> broadcast::Receiver<()> {
        self.lane(session).wake.subscribe()
    }

    async fn close(&self, session: &SessionId) -> Result<()> {
        self.lanes.remove(session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{Principal, SessionIdGenerator, Wait};
    use std::time::Duration;

    fn mk_session(gen_: &SessionIdGenerator) -> Session {
        Session::new(
            gen_.next(),
            None,
            "t".to_string(),
            Principal("anon".to_string()),
            None,
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn insert_and_get_roundtrip() {
        let store = InMemorySessionStore::new();
        let gen_ = SessionIdGenerator::new();
        let s = mk_session(&gen_);
        let id = s.id.clone();
        store.sessions.insert(s.id.clone(), s);
        assert!(store.get(&id).await.unwrap().is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_and_remove_wait() {
        let store = InMemorySessionStore::new();
        let gen_ = SessionIdGenerator::new();
        let s = mk_session(&gen_);
        let id = s.id.clone();
        store.sessions.insert(s.id.clone(), s);
        store.add_wait(&id, Wait::UserMessage).await.unwrap();
        let got = store.get(&id).await.unwrap().unwrap();
        assert!(got.waits.contains(&Wait::UserMessage));
        store.remove_wait(&id, &Wait::UserMessage).await.unwrap();
        let got = store.get(&id).await.unwrap().unwrap();
        assert!(!got.waits.contains(&Wait::UserMessage));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn list_active_excludes_terminal() {
        let store = InMemorySessionStore::new();
        let gen_ = SessionIdGenerator::new();
        let s = mk_session(&gen_);
        let id = s.id.clone();
        store.sessions.insert(s.id.clone(), s);
        store.set_status(&id, SessionStatus::Done).await.unwrap();
        assert!(store.list_active().await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn inbox_push_drain_fifo() {
        let inbox = InMemoryInbox::new();
        let id = SessionId::from_string("s_1".to_string()).unwrap();
        inbox
            .push(
                &id,
                SystemMsg::UserMessage {
                    text: "first".to_string(),
                    received_at: std::time::Instant::now(),
                },
            )
            .await
            .unwrap();
        inbox
            .push(
                &id,
                SystemMsg::UserMessage {
                    text: "second".to_string(),
                    received_at: std::time::Instant::now(),
                },
            )
            .await
            .unwrap();
        let drained = inbox.drain(&id).await.unwrap();
        assert_eq!(drained.len(), 2);
        match (&drained[0], &drained[1]) {
            (
                SystemMsg::UserMessage { text: a, .. },
                SystemMsg::UserMessage { text: b, .. },
            ) => {
                assert_eq!(a, "first");
                assert_eq!(b, "second");
            }
            _ => panic!("unexpected variants"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn inbox_subscribe_wakes_on_push() {
        let inbox = Arc::new(InMemoryInbox::new());
        let id = SessionId::from_string("s_2".to_string()).unwrap();
        let mut rx = inbox.subscribe(&id);
        let inbox2 = inbox.clone();
        let id2 = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            inbox2
                .push(
                    &id2,
                    SystemMsg::UserMessage {
                        text: "hi".to_string(),
                        received_at: std::time::Instant::now(),
                    },
                )
                .await
                .unwrap();
        });
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("did not wake")
            .expect("recv ok");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn inbox_concurrent_pushes_all_visible() {
        let inbox = Arc::new(InMemoryInbox::new());
        let id = SessionId::from_string("s_3".to_string()).unwrap();
        let mut handles = Vec::new();
        for i in 0..16 {
            let inbox = inbox.clone();
            let id = id.clone();
            handles.push(tokio::spawn(async move {
                inbox
                    .push(
                        &id,
                        SystemMsg::UserMessage {
                            text: format!("m{i}"),
                            received_at: std::time::Instant::now(),
                        },
                    )
                    .await
                    .unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let drained = inbox.drain(&id).await.unwrap();
        assert_eq!(drained.len(), 16);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn inbox_close_after_terminal_signals_receivers() {
        let store = Arc::new(InMemorySessionStore::new());
        let inbox = Arc::new(InMemoryInbox::new());
        let sid = store
            .create_root("t".to_string(), Principal("anon".to_string()), None)
            .await
            .unwrap();
        let mut rx = inbox.subscribe(&sid);
        store
            .mark_terminal_from_outcome(&sid, super::TerminalOutcome::Done)
            .await
            .unwrap();
        inbox.close(&sid).await.unwrap();
        let res = rx.recv().await;
        assert!(res.is_err(), "expected closed channel after inbox.close()");
    }
}
