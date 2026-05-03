//! Persistence traits for sessions and inboxes.
//!
//! v1 ships an in-memory implementation only (`memory.rs`). The traits are
//! sized so a SQLite-backed implementation in v2 can swap in without changing
//! callers.

pub mod memory;
pub mod notifications;
pub mod tickets;

use crate::inbox::SystemMsg;
use crate::sessions::{Principal, Session, SessionError, SessionId, SessionStatus, Wait, WaitRef};
use std::collections::HashSet;
use std::time::Instant;

pub type Result<T> = std::result::Result<T, SessionError>;

/// Terminal outcome marker (D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalOutcome {
    Done,
    Abandoned(crate::sub_agent::outcome::AbandonReason),
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    StatusChanged(SessionStatus),
    WaitAdded,
    WaitResolved,
}

#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    async fn create_root(
        &self,
        agent_label: String,
        principal: Principal,
        deadline: Option<Instant>,
    ) -> Result<SessionId>;
    async fn create_child(
        &self,
        parent: &SessionId,
        agent_label: String,
        principal: Principal,
        deadline: Option<Instant>,
    ) -> Result<SessionId>;
    async fn exists(&self, id: &SessionId) -> Result<bool>;
    async fn get(&self, id: &SessionId) -> Result<Option<Session>>;
    async fn status(&self, id: &SessionId) -> Result<Option<SessionStatus>>;
    async fn set_status(&self, id: &SessionId, status: SessionStatus) -> Result<()>;
    async fn add_wait(&self, id: &SessionId, wait: Wait) -> Result<()>;
    async fn remove_wait(&self, id: &SessionId, wait: &Wait) -> Result<()>;
    async fn waits(&self, id: &SessionId) -> Result<HashSet<Wait>>;
    async fn wait_refs(&self, id: &SessionId) -> Result<Vec<WaitRef>>;
    async fn list_active(&self) -> Result<Vec<Session>>;
    async fn mark_terminal_from_outcome(
        &self,
        id: &SessionId,
        outcome: TerminalOutcome,
    ) -> Result<()>;
    fn subscribe(&self, id: &SessionId) -> tokio::sync::broadcast::Receiver<SessionEvent>;
}

#[async_trait::async_trait]
pub trait Inbox: Send + Sync {
    async fn push(&self, session: &SessionId, msg: SystemMsg) -> Result<()>;
    async fn drain(&self, session: &SessionId) -> Result<Vec<SystemMsg>>;
    fn subscribe(&self, session: &SessionId) -> tokio::sync::broadcast::Receiver<()>;
    /// Cleanup on terminal: drops broadcast sender + removes lane (D4).
    async fn close(&self, session: &SessionId) -> Result<()>;
}
