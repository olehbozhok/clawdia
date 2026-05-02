//! Expiry sweep: transitions Pending tickets past their deadline to Expired and
//! emits `SystemMsg::ApprovalDecided{Expired}` to the owner inbox.

use crate::approvals::gateway::GatewayError;
use crate::approvals::outcome::{ApprovalOutcome, ApproverIdentity, ApproverKind};
use crate::approvals::types::TicketStatus;
use crate::inbox::SystemMsg;
use crate::persistence::Inbox;
use crate::persistence::tickets::TicketStore;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub async fn sweep_expired(
    tickets: &dyn TicketStore,
    inbox: &dyn Inbox,
    now: Instant,
) -> Result<usize, GatewayError> {
    let mut count = 0usize;
    for t in tickets.list_all()? {
        if t.status == TicketStatus::Pending && now >= t.expires_at {
            tickets.update_status(&t.id, TicketStatus::Expired)?;
            inbox
                .push(
                    &t.session_id,
                    SystemMsg::ApprovalDecided {
                        ticket_id: t.id.clone(),
                        action_kind: t.action_kind.clone(),
                        decision: ApprovalOutcome::Expired,
                        approver: ApproverIdentity {
                            kind: ApproverKind::LocalKey {
                                key_id: "system".into(),
                            },
                            roles: vec![],
                        },
                        decided_at: now,
                    },
                )
                .await?;
            count += 1;
        }
    }
    Ok(count)
}

/// Spawn the expiry sweep loop. The loop exits when `cancel` is triggered,
/// so callers must hold the token (typically a runtime-wide shutdown token)
/// and cancel it during graceful shutdown. Without cancellation the task
/// can only be torn down via `JoinHandle::abort`.
pub fn spawn(
    tickets: Arc<dyn TicketStore>,
    inbox: Arc<dyn Inbox>,
    interval: Duration,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(interval) => {
                    let _ = sweep_expired(&*tickets, &*inbox, Instant::now()).await;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::types::{Ticket, TicketId};
    use crate::persistence::memory::InMemoryInbox;
    use crate::persistence::tickets::InMemoryTicketStore;
    use crate::sessions::SessionId;

    fn mk_ticket(id: &str, status: TicketStatus, expires_at: Instant) -> Ticket {
        Ticket {
            id: TicketId(id.into()),
            session_id: SessionId::for_test("s1"),
            action_kind: "act".into(),
            args: serde_json::json!({}),
            args_hash: "sha256:00".into(),
            reason: "r".into(),
            hint: None,
            status,
            decision: None,
            created_at: Instant::now(),
            expires_at,
            decided_at: None,
            consumed_at: None,
        }
    }

    #[tokio::test]
    async fn expiry_sweep_transitions_pending_to_expired_after_deadline() {
        let store = InMemoryTicketStore::new();
        let inbox = InMemoryInbox::new();
        let now = Instant::now();
        store
            .create(
                mk_ticket("t1", TicketStatus::Pending, now - Duration::from_secs(1)),
                "k1",
            )
            .unwrap();

        let count = sweep_expired(&*store, &inbox, now).await.unwrap();
        assert_eq!(count, 1);
        let all = store.list_all().unwrap();
        assert_eq!(all[0].status, TicketStatus::Expired);
    }

    #[tokio::test]
    async fn expiry_sweep_emits_approval_decided_expired() {
        let store = InMemoryTicketStore::new();
        let inbox = InMemoryInbox::new();
        let now = Instant::now();
        store
            .create(
                mk_ticket("t2", TicketStatus::Pending, now - Duration::from_secs(1)),
                "k2",
            )
            .unwrap();

        sweep_expired(&*store, &inbox, now).await.unwrap();
        let mut msgs = inbox.drain(&SessionId::for_test("s1")).await.unwrap();
        assert_eq!(msgs.len(), 1);
        match msgs.remove(0) {
            SystemMsg::ApprovalDecided {
                decision: ApprovalOutcome::Expired,
                ..
            } => {}
            _ => panic!("expected ApprovalDecided with Expired"),
        }
    }

    #[tokio::test]
    async fn expiry_sweep_does_not_touch_approved_or_consumed() {
        let store = InMemoryTicketStore::new();
        let inbox = InMemoryInbox::new();
        let now = Instant::now();
        store
            .create(
                mk_ticket("t3", TicketStatus::Approved, now - Duration::from_secs(1)),
                "k3",
            )
            .unwrap();
        store
            .create(
                mk_ticket("t4", TicketStatus::Consumed, now - Duration::from_secs(1)),
                "k4",
            )
            .unwrap();
        store
            .create(
                mk_ticket("t5", TicketStatus::Pending, now + Duration::from_secs(3600)),
                "k5",
            )
            .unwrap();

        let count = sweep_expired(&*store, &inbox, now).await.unwrap();
        assert_eq!(count, 0);
        let all = store.list_all().unwrap();
        assert!(all.iter().all(|t| t.status != TicketStatus::Expired));
    }

    #[tokio::test]
    async fn spawn_loop_exits_on_cancel() {
        let store: Arc<dyn TicketStore> = InMemoryTicketStore::new();
        let inbox: Arc<dyn Inbox> = Arc::new(InMemoryInbox::new());
        let cancel = CancellationToken::new();
        let handle = spawn(store, inbox, Duration::from_millis(50), cancel.clone());
        cancel.cancel();
        // Bounded wait: handle must complete promptly after cancellation.
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("expiry task should exit on cancel")
            .unwrap();
    }

    #[tokio::test]
    async fn expiry_sweep_skips_pending_within_ttl() {
        let store = InMemoryTicketStore::new();
        let inbox = InMemoryInbox::new();
        let now = Instant::now();
        store
            .create(
                mk_ticket("t6", TicketStatus::Pending, now + Duration::from_secs(3600)),
                "k6",
            )
            .unwrap();

        let count = sweep_expired(&*store, &inbox, now).await.unwrap();
        assert_eq!(count, 0);
        let all = store.list_all().unwrap();
        assert_eq!(all[0].status, TicketStatus::Pending);
    }
}
