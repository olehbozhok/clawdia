//! In-memory notification store. FIFO ordering preserved via Mutex<Vec<_>>;
//! a DashMap would lose insertion order. v1 is single-process.

use crate::notifications::types::Notification;
use crate::sessions::SessionId;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use thiserror::Error;

pub trait NotificationStore: Send + Sync {
    fn record(&self, n: Notification) -> Result<(), NotificationStoreError>;
    fn list(&self, since: Option<SystemTime>) -> Result<Vec<Notification>, NotificationStoreError>;
    /// Returns notifications where any of these hold (design §17.5 session
    /// linkage):
    /// - `n.emitted_by == *sid`
    /// - `n.refs.session_id == Some(*sid)`
    /// - `n.refs.child_session_id == Some(*sid)`
    fn list_by_session(
        &self,
        sid: &SessionId,
    ) -> Result<Vec<Notification>, NotificationStoreError>;
}

#[derive(Debug, Error)]
pub enum NotificationStoreError {
    #[error("duplicate notification id: {0}")]
    Duplicate(String),
}

#[derive(Default)]
pub struct InMemoryNotificationStore {
    notifications: Mutex<Vec<Notification>>,
}

impl InMemoryNotificationStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl NotificationStore for InMemoryNotificationStore {
    fn record(&self, n: Notification) -> Result<(), NotificationStoreError> {
        let mut guard = self.notifications.lock().expect("lock poisoned");
        if guard.iter().any(|existing| existing.id == n.id) {
            return Err(NotificationStoreError::Duplicate(n.id.0.clone()));
        }
        guard.push(n);
        Ok(())
    }

    fn list(
        &self,
        since: Option<SystemTime>,
    ) -> Result<Vec<Notification>, NotificationStoreError> {
        let guard = self.notifications.lock().expect("lock poisoned");
        Ok(match since {
            Some(t) => guard.iter().filter(|n| n.emitted_at > t).cloned().collect(),
            None => guard.clone(),
        })
    }

    fn list_by_session(
        &self,
        sid: &SessionId,
    ) -> Result<Vec<Notification>, NotificationStoreError> {
        let guard = self.notifications.lock().expect("lock poisoned");
        Ok(guard
            .iter()
            .filter(|n| {
                n.emitted_by == *sid
                    || n.refs.session_id.as_ref() == Some(sid)
                    || n.refs.child_session_id.as_ref() == Some(sid)
            })
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::types::{NotificationId, NotificationRefs, Severity};
    use std::time::Duration;

    fn mk(id: &str, emitted_by: &str, emitted_at: SystemTime) -> Notification {
        Notification {
            id: NotificationId(id.into()),
            severity: Severity::Info,
            subject: "subj".into(),
            body: "body".into(),
            refs: NotificationRefs::default(),
            emitted_by: SessionId::for_test(emitted_by),
            emitted_at,
        }
    }

    #[test]
    fn record_and_list_returns_fifo_order() {
        let store = InMemoryNotificationStore::new();
        let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let t3 = SystemTime::UNIX_EPOCH + Duration::from_secs(3);
        store.record(mk("a", "s1", t1)).unwrap();
        store.record(mk("b", "s1", t2)).unwrap();
        store.record(mk("c", "s1", t3)).unwrap();
        let all = store.list(None).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].emitted_at, t1);
        assert_eq!(all[2].emitted_at, t3);
    }

    #[test]
    fn list_with_since_filters_strictly_after() {
        let store = InMemoryNotificationStore::new();
        let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let t3 = SystemTime::UNIX_EPOCH + Duration::from_secs(3);
        store.record(mk("a", "s1", t1)).unwrap();
        store.record(mk("b", "s1", t2)).unwrap();
        store.record(mk("c", "s1", t3)).unwrap();
        let result = store.list(Some(t2)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].emitted_at, t3);
    }

    #[test]
    fn list_with_none_returns_all() {
        let store = InMemoryNotificationStore::new();
        store
            .record(mk("a", "s1", SystemTime::UNIX_EPOCH))
            .unwrap();
        store
            .record(mk("b", "s1", SystemTime::UNIX_EPOCH + Duration::from_secs(1)))
            .unwrap();
        assert_eq!(store.list(None).unwrap().len(), 2);
    }

    #[test]
    fn list_by_session_filters_by_emitted_by() {
        let store = InMemoryNotificationStore::new();
        let s1 = SessionId::for_test("s1");
        let t = SystemTime::UNIX_EPOCH;
        store.record(mk("a", "s1", t)).unwrap();
        store.record(mk("b", "s2", t)).unwrap();
        let result = store.list_by_session(&s1).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.0, "a");
    }

    #[test]
    fn list_by_session_filters_by_refs_session_id() {
        let store = InMemoryNotificationStore::new();
        let s1 = SessionId::for_test("s1");
        let mut n = mk("a", "s2", SystemTime::UNIX_EPOCH);
        n.refs.session_id = Some(s1.clone());
        store.record(n).unwrap();
        let result = store.list_by_session(&s1).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.0, "a");
    }

    #[test]
    fn list_by_session_filters_by_refs_child_session_id() {
        let store = InMemoryNotificationStore::new();
        let s1 = SessionId::for_test("s1");
        let mut n = mk("a", "s2", SystemTime::UNIX_EPOCH);
        n.refs.child_session_id = Some(s1.clone());
        store.record(n).unwrap();
        let result = store.list_by_session(&s1).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.0, "a");
    }

    #[test]
    fn record_rejects_duplicate_id() {
        let store = InMemoryNotificationStore::new();
        store
            .record(mk("dup", "s1", SystemTime::UNIX_EPOCH))
            .unwrap();
        let err = store
            .record(mk("dup", "s2", SystemTime::UNIX_EPOCH))
            .unwrap_err();
        match err {
            NotificationStoreError::Duplicate(id) => assert_eq!(id, "dup"),
        }
    }
}
