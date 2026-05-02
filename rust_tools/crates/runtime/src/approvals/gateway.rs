//! Approval gateway: request, decide, and the six-step runtime gate.
//!
//! All runtime gates fire BEFORE Cedar `context.approval` is populated. Cedar
//! is the policy layer; the runtime gate is the integrity layer.

use crate::approvals::canonical::canonical_hash;
use crate::approvals::correlation::correlation_key;
use crate::approvals::outcome::{ApprovalOutcome, ApproverIdentity};
use crate::approvals::signing::{SignError, sign, verify};
use crate::approvals::types::{
    ApprovalRequest, Decision, Ticket, TicketId, TicketStatus,
};
use crate::inbox::SystemMsg;
use crate::persistence::tickets::{TicketStore, TicketStoreError};
use crate::persistence::{Inbox, SessionStore};
use crate::sessions::{SessionError, SessionId, Wait};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("canonical: {0}")]
    Canonical(#[from] crate::approvals::canonical::CanonicalError),
    #[error("store: {0}")]
    Store(#[from] TicketStoreError),
    #[error("session: {0}")]
    Session(#[from] SessionError),
    #[error("not approved")]
    NotApproved,
    #[error("ticket already decided: current status {current_status:?}")]
    AlreadyDecided { current_status: TicketStatus },
    #[error("expired")]
    Expired,
    #[error("consumed")]
    Consumed,
    #[error("session mismatch")]
    SessionMismatch,
    #[error("args drift")]
    ArgsDrift,
    #[error("bad signature")]
    BadSignature,
    #[error("ticket not found")]
    NotFound,
}

type IdGen = Box<dyn Fn() -> TicketId + Send + Sync>;
type Clock = Box<dyn Fn() -> Instant + Send + Sync>;

pub struct ApprovalGateway {
    tickets: Arc<dyn TicketStore>,
    sessions: Arc<dyn SessionStore>,
    inbox: Arc<dyn Inbox>,
    hmac_key: Vec<u8>,
    id_gen: IdGen,
    clock: Clock,
}

#[derive(Debug)]
pub struct GateOk {
    pub ticket: Ticket,
}

impl ApprovalGateway {
    pub fn new(
        tickets: Arc<dyn TicketStore>,
        sessions: Arc<dyn SessionStore>,
        inbox: Arc<dyn Inbox>,
        hmac_key: Vec<u8>,
    ) -> Self {
        Self {
            tickets,
            sessions,
            inbox,
            hmac_key,
            id_gen: Box::new(default_id_gen),
            clock: Box::new(Instant::now),
        }
    }

    pub fn with_id_gen<F>(mut self, f: F) -> Self
    where
        F: Fn() -> TicketId + Send + Sync + 'static,
    {
        self.id_gen = Box::new(f);
        self
    }

    pub fn with_clock<F>(mut self, f: F) -> Self
    where
        F: Fn() -> Instant + Send + Sync + 'static,
    {
        self.clock = Box::new(f);
        self
    }

    pub async fn request(&self, req: ApprovalRequest) -> Result<Ticket, GatewayError> {
        let key = correlation_key(&req.requester, &req.action_kind, &req.args)?;
        if let Some(existing) = self.tickets.find_by_correlation(&key)? {
            return Ok(existing);
        }
        let now = (self.clock)();
        let args_hash = canonical_hash(&req.args)?;
        let ticket = Ticket {
            id: (self.id_gen)(),
            session_id: req.requester,
            action_kind: req.action_kind,
            args: req.args,
            args_hash,
            reason: req.reason,
            hint: req.hint,
            status: TicketStatus::Pending,
            decision: None,
            created_at: now,
            expires_at: now + req.ttl,
            decided_at: None,
            consumed_at: None,
        };
        self.tickets.create(ticket.clone(), &key)?;
        Ok(ticket)
    }

    pub async fn decide(
        &self,
        id: &TicketId,
        outcome: ApprovalOutcome,
        approver: ApproverIdentity,
        signature: Vec<u8>,
    ) -> Result<(), GatewayError> {
        let ticket = self.tickets.get(id)?.ok_or(GatewayError::NotFound)?;
        if ticket.status != TicketStatus::Pending {
            return Err(GatewayError::AlreadyDecided {
                current_status: ticket.status,
            });
        }
        verify(&self.hmac_key, id, &ticket.args_hash, &outcome, &signature)
            .map_err(|_: SignError| GatewayError::BadSignature)?;
        let new_status = match &outcome {
            ApprovalOutcome::Approved => TicketStatus::Approved,
            ApprovalOutcome::Denied { .. } => TicketStatus::Denied,
            ApprovalOutcome::Expired => TicketStatus::Expired,
        };
        let reason = match &outcome {
            ApprovalOutcome::Denied { reason } => Some(reason.clone()),
            _ => None,
        };
        self.tickets.record_decision(
            id,
            Decision {
                approver: approver.clone(),
                signature,
                reason,
            },
        )?;
        self.tickets.update_status(id, new_status)?;
        self.inbox
            .push(
                &ticket.session_id,
                SystemMsg::ApprovalDecided {
                    ticket_id: id.clone(),
                    action_kind: ticket.action_kind.clone(),
                    decision: outcome.clone(),
                    approver,
                    decided_at: (self.clock)(),
                },
            )
            .await?;
        // D14/D5: resolve session wait via SessionStore::remove_wait. Best-effort —
        // a non-existent or already-removed wait is not an error.
        let _ = self
            .sessions
            .remove_wait(&ticket.session_id, &Wait::Approval(id.clone()))
            .await;
        Ok(())
    }

    /// D9: CLI/TUI convenience — sign with the gateway's local HMAC key, then
    /// call `decide`. Web/JWT path uses `decide` directly with a pre-signed value.
    pub async fn decide_local(
        &self,
        id: &TicketId,
        outcome: ApprovalOutcome,
        approver: ApproverIdentity,
    ) -> Result<(), GatewayError> {
        let ticket = self.tickets.get(id)?.ok_or(GatewayError::NotFound)?;
        let signature = sign(&self.hmac_key, id, &ticket.args_hash, &outcome)
            .map_err(|_| GatewayError::BadSignature)?;
        self.decide(id, outcome, approver, signature).await
    }

    /// Six-step runtime gate. On success transitions to `Consumed` atomically
    /// and returns the stored ticket (caller uses `ticket.args` for execution).
    pub async fn gate_call(
        &self,
        id: &TicketId,
        calling_session: &SessionId,
        current_args: &Value,
    ) -> Result<GateOk, GatewayError> {
        let ticket = self.tickets.get(id)?.ok_or(GatewayError::NotFound)?;
        if &ticket.session_id != calling_session {
            return Err(GatewayError::SessionMismatch);
        }
        // D19: terminal-non-actionable statuses short-circuit BEFORE signature
        // verification. Synthesized Expired tickets carry an empty signature.
        match ticket.status {
            TicketStatus::Approved => {}
            TicketStatus::Consumed => return Err(GatewayError::Consumed),
            TicketStatus::Expired | TicketStatus::Orphaned => return Err(GatewayError::Expired),
            _ => return Err(GatewayError::NotApproved),
        }
        let cur_hash = canonical_hash(current_args)?;
        if cur_hash != ticket.args_hash {
            return Err(GatewayError::ArgsDrift);
        }
        if (self.clock)() >= ticket.expires_at {
            return Err(GatewayError::Expired);
        }
        let decision = ticket.decision.as_ref().ok_or(GatewayError::NotApproved)?;
        verify(
            &self.hmac_key,
            id,
            &ticket.args_hash,
            &ApprovalOutcome::Approved,
            &decision.signature,
        )
        .map_err(|_| GatewayError::BadSignature)?;
        self.tickets.update_status(id, TicketStatus::Consumed)?;
        Ok(GateOk { ticket })
    }
}

fn default_id_gen() -> TicketId {
    use rand::RngCore;
    let mut b = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut b);
    TicketId(format!("tk_{}", hex::encode(b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::outcome::{ApproverIdentity, ApproverKind};
    use crate::persistence::memory::{InMemoryInbox, InMemorySessionStore};
    use crate::persistence::tickets::InMemoryTicketStore;
    use crate::sessions::Principal;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;

    fn approver() -> ApproverIdentity {
        ApproverIdentity {
            kind: ApproverKind::LocalKey {
                key_id: "k1".into(),
            },
            roles: vec!["campaign_owner".into()],
        }
    }

    struct Harness {
        gw: ApprovalGateway,
        sessions: Arc<InMemorySessionStore>,
        inbox: Arc<InMemoryInbox>,
        tickets: Arc<InMemoryTicketStore>,
        clock: Arc<Mutex<Instant>>,
    }

    fn harness() -> Harness {
        let tickets = InMemoryTicketStore::new();
        let sessions = InMemorySessionStore::arc();
        let inbox = InMemoryInbox::arc();
        let clock = Arc::new(Mutex::new(Instant::now()));
        let counter = AtomicU64::new(0);
        let clock_for_gw = clock.clone();
        let gw = ApprovalGateway::new(
            tickets.clone() as Arc<dyn TicketStore>,
            sessions.clone() as Arc<dyn SessionStore>,
            inbox.clone() as Arc<dyn Inbox>,
            vec![0xABu8; 32],
        )
        .with_id_gen(move || {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            TicketId(format!("tk_{n}"))
        })
        .with_clock(move || *clock_for_gw.lock().unwrap());
        Harness {
            gw,
            sessions,
            inbox,
            tickets,
            clock,
        }
    }

    async fn make_session(h: &Harness) -> SessionId {
        h.sessions
            .create_root("orchestrator".into(), Principal("anon".into()), None)
            .await
            .unwrap()
    }

    fn req(sid: SessionId, args: Value) -> ApprovalRequest {
        ApprovalRequest {
            action_kind: "doc.publish_live".into(),
            args,
            reason: "go live".into(),
            hint: None,
            requester: sid,
            ttl: Duration::from_secs(60),
            metadata: json!({}),
        }
    }

    #[tokio::test]
    async fn request_creates_pending_ticket_with_args_hash() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h.gw.request(req(sid, json!({"x": 1}))).await.unwrap();
        assert_eq!(t.status, TicketStatus::Pending);
        assert!(t.args_hash.starts_with("sha256:"));
    }

    #[tokio::test]
    async fn request_idempotent_via_correlation() {
        let h = harness();
        let sid = make_session(&h).await;
        let t1 = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        let t2 = h.gw.request(req(sid, json!({"x": 1}))).await.unwrap();
        assert_eq!(t1.id, t2.id);
    }

    #[tokio::test]
    async fn decide_records_decision_and_emits_inbox() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        let stored = h.tickets.get(&t.id).unwrap().unwrap();
        assert_eq!(stored.status, TicketStatus::Approved);
        assert!(stored.decision.is_some());
        let drained = h.inbox.drain(&sid).await.unwrap();
        assert_eq!(drained.len(), 1);
        matches!(drained[0], SystemMsg::ApprovalDecided { .. });
    }

    #[tokio::test]
    async fn decide_resolves_session_wait_approval() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        h.sessions
            .add_wait(&sid, Wait::Approval(t.id.clone()))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        let waits = h.sessions.waits(&sid).await.unwrap();
        assert!(!waits.contains(&Wait::Approval(t.id)));
    }

    #[tokio::test]
    async fn decide_on_non_pending_returns_already_decided() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid, json!({"x": 1})))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        let err = h
            .gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            GatewayError::AlreadyDecided {
                current_status: TicketStatus::Approved
            }
        ));
    }

    #[tokio::test]
    async fn gate_call_rejects_when_pending() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        let err = h.gw.gate_call(&t.id, &sid, &json!({"x": 1})).await.unwrap_err();
        assert!(matches!(err, GatewayError::NotApproved));
    }

    #[tokio::test]
    async fn gate_call_rejects_when_args_drift() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        let err = h.gw.gate_call(&t.id, &sid, &json!({"x": 2})).await.unwrap_err();
        assert!(matches!(err, GatewayError::ArgsDrift));
    }

    #[tokio::test]
    async fn gate_call_rejects_when_expired() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        // Advance virtual clock past expires_at.
        *h.clock.lock().unwrap() = Instant::now() + Duration::from_secs(3600);
        let err = h.gw.gate_call(&t.id, &sid, &json!({"x": 1})).await.unwrap_err();
        assert!(matches!(err, GatewayError::Expired));
    }

    #[tokio::test]
    async fn gate_call_rejects_when_session_mismatch() {
        let h = harness();
        let sid = make_session(&h).await;
        let other = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid, json!({"x": 1})))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        let err = h
            .gw
            .gate_call(&t.id, &other, &json!({"x": 1}))
            .await
            .unwrap_err();
        assert!(matches!(err, GatewayError::SessionMismatch));
    }

    #[tokio::test]
    async fn gate_call_rejects_when_signature_invalid() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        // Tamper signature in the store.
        let mut stored = h.tickets.get(&t.id).unwrap().unwrap();
        let mut bad = stored.decision.clone().unwrap();
        bad.signature[0] ^= 0xFF;
        stored.decision = Some(bad.clone());
        h.tickets.record_decision(&t.id, bad).unwrap();
        let err = h
            .gw
            .gate_call(&t.id, &sid, &json!({"x": 1}))
            .await
            .unwrap_err();
        assert!(matches!(err, GatewayError::BadSignature));
    }

    #[tokio::test]
    async fn gate_call_consumes_atomically() {
        let h = harness();
        let sid = make_session(&h).await;
        let t = h
            .gw
            .request(req(sid.clone(), json!({"x": 1})))
            .await
            .unwrap();
        h.gw
            .decide_local(&t.id, ApprovalOutcome::Approved, approver())
            .await
            .unwrap();
        h.gw.gate_call(&t.id, &sid, &json!({"x": 1})).await.unwrap();
        let err = h
            .gw
            .gate_call(&t.id, &sid, &json!({"x": 1}))
            .await
            .unwrap_err();
        assert!(matches!(err, GatewayError::Consumed));
    }
}
