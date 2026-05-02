//! Orphan sweep: marks Pending tickets Orphaned when owning session terminates.
//! Approved tickets stay actionable until consumed; only Pending → Orphaned.

use crate::approvals::types::TicketStatus;
use crate::persistence::tickets::{TicketStore, TicketStoreError};
use crate::sessions::SessionId;

pub fn sweep_orphans(
    tickets: &dyn TicketStore,
    sid: &SessionId,
) -> Result<usize, TicketStoreError> {
    let mut count = 0;
    for t in tickets.list_by_session(sid)? {
        if matches!(t.status, TicketStatus::Pending) {
            tickets.update_status(&t.id, TicketStatus::Orphaned)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::types::{Ticket, TicketId};
    use crate::persistence::tickets::InMemoryTicketStore;
    use std::time::Instant;

    fn mk_ticket(id: &str, sid: &str, status: TicketStatus) -> Ticket {
        let now = Instant::now();
        Ticket {
            id: TicketId(id.into()),
            session_id: SessionId::for_test(sid),
            action_kind: "act".into(),
            args: serde_json::json!({}),
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
    fn orphan_sweep_marks_pending_tickets_orphaned_when_session_abandoned() {
        let store = InMemoryTicketStore::new();
        let sid = SessionId::for_test("s1");
        store
            .create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        store
            .create(mk_ticket("t2", "s1", TicketStatus::Pending), "k2")
            .unwrap();

        let count = sweep_orphans(&*store, &sid).unwrap();
        assert_eq!(count, 2);
        for t in store.list_by_session(&sid).unwrap() {
            assert!(matches!(t.status, TicketStatus::Orphaned));
        }
    }

    #[test]
    fn orphan_sweep_leaves_approved_tickets_alone() {
        let store = InMemoryTicketStore::new();
        let sid = SessionId::for_test("s1");
        store
            .create(mk_ticket("t1", "s1", TicketStatus::Approved), "k1")
            .unwrap();
        store
            .create(mk_ticket("t2", "s1", TicketStatus::Pending), "k2")
            .unwrap();

        let count = sweep_orphans(&*store, &sid).unwrap();
        assert_eq!(count, 1);
        let listed = store.list_by_session(&sid).unwrap();
        let approved = listed.iter().find(|t| t.id.0 == "t1").unwrap();
        assert!(matches!(approved.status, TicketStatus::Approved));
        let orphaned = listed.iter().find(|t| t.id.0 == "t2").unwrap();
        assert!(matches!(orphaned.status, TicketStatus::Orphaned));
    }

    #[test]
    fn orphan_sweep_emits_no_inbox_message() {
        // Signature has no Inbox parameter — orphan sweep is fire-and-forget.
        let store = InMemoryTicketStore::new();
        let sid = SessionId::for_test("s1");
        store
            .create(mk_ticket("t1", "s1", TicketStatus::Pending), "k1")
            .unwrap();
        assert_eq!(sweep_orphans(&*store, &sid).unwrap(), 1);
    }
}
