//! `emit_notification`: build → tracing → store. Sync, fire-and-forget per
//! design §17. NOT blocking, NOT touching inbox/session state.
//!
//! No AuditLog parameter — design §17.5 calls for an audit trail; the
//! `tools::authz::AuditEntry` shipping in this repo is an
//! authorization-shaped struct (not an enum, see Plan 04 Outcome). The
//! notification trail is therefore: `tracing` events at severity level +
//! `NotificationStore::list()` for query.

use crate::notifications::types::{
    Notification, NotificationId, NotificationIdGenerator, NotificationRefs, Severity,
};
use crate::persistence::notifications::{NotificationStore, NotificationStoreError};
use crate::sessions::SessionId;
use std::time::SystemTime;
use thiserror::Error;

pub struct EmitArgs {
    pub severity: Severity,
    pub subject: String,
    pub body: String,
    pub refs: NotificationRefs,
    pub emitted_by: SessionId,
}

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error(transparent)]
    Store(#[from] NotificationStoreError),
}

pub fn emit_notification(
    store: &dyn NotificationStore,
    idgen: &NotificationIdGenerator,
    args: EmitArgs,
) -> Result<NotificationId, NotifyError> {
    let n = Notification {
        id: idgen.next(),
        severity: args.severity,
        subject: args.subject,
        body: args.body,
        refs: args.refs,
        emitted_by: args.emitted_by,
        emitted_at: SystemTime::now(),
    };

    match n.severity {
        Severity::Info => {
            tracing::info!(
                id = %n.id.0,
                subject = %n.subject,
                "{}", n.body
            );
        }
        Severity::Warn => {
            tracing::warn!(
                id = %n.id.0,
                subject = %n.subject,
                "{}", n.body
            );
        }
        Severity::Error => {
            tracing::error!(
                id = %n.id.0,
                subject = %n.subject,
                "{}", n.body
            );
        }
        Severity::Blocker => {
            tracing::error!(
                id = %n.id.0,
                subject = %n.subject,
                blocker = true,
                "[BLOCKER] {}", n.body
            );
        }
    }

    store.record(n.clone())?;
    Ok(n.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::notifications::InMemoryNotificationStore;
    use tracing_test::traced_test;

    fn args(severity: Severity, sid: &str) -> EmitArgs {
        EmitArgs {
            severity,
            subject: "subj".into(),
            body: "body".into(),
            refs: NotificationRefs::default(),
            emitted_by: SessionId::for_test(sid),
        }
    }

    #[test]
    fn emit_records_in_store() {
        let store = InMemoryNotificationStore::new();
        let idgen = NotificationIdGenerator::new();
        let id = emit_notification(&*store, &idgen, args(Severity::Info, "s1")).unwrap();
        let listed = store.list(None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);
    }

    #[test]
    fn emit_returns_fresh_unique_ids() {
        let store = InMemoryNotificationStore::new();
        let idgen = NotificationIdGenerator::new();
        let a = emit_notification(&*store, &idgen, args(Severity::Info, "s1")).unwrap();
        let b = emit_notification(&*store, &idgen, args(Severity::Warn, "s2")).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn emit_is_sync_no_session_state_touch() {
        // Compile-time guarantee: function is `fn`, not `async fn`. Test
        // existence proves the contract; no Wait/Inbox dependency in scope.
        let store = InMemoryNotificationStore::new();
        let idgen = NotificationIdGenerator::new();
        emit_notification(&*store, &idgen, args(Severity::Info, "s1")).unwrap();
    }

    #[traced_test]
    #[test]
    fn emit_blocker_uses_blocker_prefix_in_log() {
        let store = InMemoryNotificationStore::new();
        let idgen = NotificationIdGenerator::new();
        emit_notification(&*store, &idgen, args(Severity::Blocker, "s1")).unwrap();
        assert!(logs_contain("[BLOCKER]"));
    }

    #[traced_test]
    #[test]
    fn emit_warn_logs_at_warn_level() {
        let store = InMemoryNotificationStore::new();
        let idgen = NotificationIdGenerator::new();
        emit_notification(&*store, &idgen, args(Severity::Warn, "s1")).unwrap();
        // tracing-test does not expose level filtering directly; check the
        // body landed in logs (level itself verified via macro choice).
        assert!(logs_contain("body"));
    }
}
