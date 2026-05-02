//! Ticket persistence trait + in-memory implementation.
//!
//! D6: `list_all` lives in the trait from the start — the expiry sweep
//! (Task 16) iterates every ticket regardless of session.

use crate::approvals::types::{Decision, Ticket, TicketId, TicketStatus};
use crate::sessions::SessionId;
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TicketStoreError {
    #[error("ticket {0:?} not found")]
    NotFound(TicketId),
    #[error("duplicate ticket {0:?}")]
    Duplicate(TicketId),
}

pub type Result<T> = std::result::Result<T, TicketStoreError>;

pub trait TicketStore: Send + Sync {
    fn create(&self, t: Ticket, correlation_key: &str) -> Result<()>;
    fn get(&self, id: &TicketId) -> Result<Option<Ticket>>;
    fn find_by_correlation(&self, key: &str) -> Result<Option<Ticket>>;
    fn list_by_session(&self, sid: &SessionId) -> Result<Vec<Ticket>>;
    fn list_all(&self) -> Result<Vec<Ticket>>;
    fn update_status(&self, id: &TicketId, status: TicketStatus) -> Result<()>;
    fn record_decision(&self, id: &TicketId, decision: Decision) -> Result<()>;
}

#[derive(Default)]
pub struct InMemoryTicketStore {
    tickets: DashMap<TicketId, Ticket>,
    by_correlation: DashMap<String, TicketId>,
}

impl InMemoryTicketStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl TicketStore for InMemoryTicketStore {
    fn create(&self, t: Ticket, correlation_key: &str) -> Result<()> {
        if self.tickets.contains_key(&t.id) {
            return Err(TicketStoreError::Duplicate(t.id.clone()));
        }
        self.by_correlation
            .insert(correlation_key.to_string(), t.id.clone());
        self.tickets.insert(t.id.clone(), t);
        Ok(())
    }

    fn get(&self, id: &TicketId) -> Result<Option<Ticket>> {
        Ok(self.tickets.get(id).map(|r| r.clone()))
    }

    fn find_by_correlation(&self, key: &str) -> Result<Option<Ticket>> {
        Ok(self
            .by_correlation
            .get(key)
            .and_then(|id| self.tickets.get(&*id).map(|r| r.clone())))
    }

    fn list_by_session(&self, sid: &SessionId) -> Result<Vec<Ticket>> {
        Ok(self
            .tickets
            .iter()
            .filter(|r| &r.session_id == sid)
            .map(|r| r.clone())
            .collect())
    }

    fn list_all(&self) -> Result<Vec<Ticket>> {
        Ok(self.tickets.iter().map(|r| r.clone()).collect())
    }

    fn update_status(&self, id: &TicketId, status: TicketStatus) -> Result<()> {
        let mut e = self
            .tickets
            .get_mut(id)
            .ok_or_else(|| TicketStoreError::NotFound(id.clone()))?;
        e.status = status;
        Ok(())
    }

    fn record_decision(&self, id: &TicketId, decision: Decision) -> Result<()> {
        let mut e = self
            .tickets
            .get_mut(id)
            .ok_or_else(|| TicketStoreError::NotFound(id.clone()))?;
        e.decision = Some(decision);
        e.decided_at = Some(std::time::Instant::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::outcome::{ApproverIdentity, ApproverKind};
    use serde_json::json;
    use std::time::Instant;

    fn mk_ticket(id: &str, sid: &str, status: TicketStatus) -> Ticket {
        let now = Instant::now();
        Ticket {
            id: TicketId(id.into()),
            session_id: SessionId::for_test(sid),
            action_kind: "act".into(),
            args: json!({}),
            args_hash: "sha256:00".into(),
            reason: "r".into(),
            hint: None,
            status,
            decision: None,
            created_at: now,
            expires_at: now,
            decided_at: None,
            consumed_at: None,
        }
    }

    #[test]
    fn create_then_get() {
        let s = InMemoryTicketStore::default();
        s.create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        let got = s.get(&TicketId("t1".into())).unwrap().unwrap();
        assert_eq!(got.id.0, "t1");
    }

    #[test]
    fn find_by_correlation_idempotent() {
        let s = InMemoryTicketStore::default();
        s.create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        let got = s.find_by_correlation("k1").unwrap().unwrap();
        assert_eq!(got.id.0, "t1");
    }

    #[test]
    fn update_status_transitions_pending_to_approved() {
        let s = InMemoryTicketStore::default();
        s.create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        s.update_status(&TicketId("t1".into()), TicketStatus::Approved)
            .unwrap();
        assert_eq!(
            s.get(&TicketId("t1".into())).unwrap().unwrap().status,
            TicketStatus::Approved
        );
    }

    #[test]
    fn record_decision_persists_decision() {
        let s = InMemoryTicketStore::default();
        s.create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        let dec = Decision {
            approver: ApproverIdentity {
                kind: ApproverKind::LocalKey { key_id: "k".into() },
                roles: vec![],
            },
            signature: vec![1, 2, 3],
            reason: None,
        };
        s.record_decision(&TicketId("t1".into()), dec).unwrap();
        let t = s.get(&TicketId("t1".into())).unwrap().unwrap();
        assert!(t.decision.is_some());
        assert!(t.decided_at.is_some());
    }

    #[test]
    fn list_by_session_filters() {
        let s = InMemoryTicketStore::default();
        s.create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        s.create(mk_ticket("t2", "s2", TicketStatus::Pending), "k2")
            .unwrap();
        s.create(mk_ticket("t3", "s1", TicketStatus::Approved), "k3")
            .unwrap();
        let list = s.list_by_session(&SessionId::for_test("s1")).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn list_all_returns_every_ticket() {
        let s = InMemoryTicketStore::default();
        s.create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        s.create(mk_ticket("t2", "s2", TicketStatus::Pending), "k2")
            .unwrap();
        assert_eq!(s.list_all().unwrap().len(), 2);
    }

    #[test]
    fn unknown_id_returns_none() {
        let s = InMemoryTicketStore::default();
        assert!(s.get(&TicketId("nope".into())).unwrap().is_none());
    }

    #[test]
    fn duplicate_create_rejected() {
        let s = InMemoryTicketStore::default();
        s.create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        let err = s
            .create(mk_ticket("t1", "s1", TicketStatus::Pending), "kx")
            .unwrap_err();
        matches!(err, TicketStoreError::Duplicate(_));
    }
}
