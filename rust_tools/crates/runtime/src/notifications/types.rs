//! Notification core types. `Severity` is canonical here (D10). The design
//! doc spells `emitted_at: Instant`; we deviate to `SystemTime` because
//! notifications must be persisted and rendered to humans, and `Instant` is
//! neither serde-able nor wall-clock.

use crate::approvals::types::TicketId;
use crate::sessions::SessionId;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationId(pub String);

impl std::fmt::Display for NotificationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Sequential `NotificationId` generator. Single-process, lock-free.
/// Mirrors `SessionIdGenerator`: `n_<base36 counter>`. D18: no `ulid`.
#[derive(Debug, Default)]
pub struct NotificationIdGenerator {
    counter: AtomicU64,
}

impl NotificationIdGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next(&self) -> NotificationId {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        NotificationId(format!("n_{}", base36(n)))
    }
}

fn base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::with_capacity(13);
    while n > 0 {
        out.push(ALPHABET[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).expect("base36 alphabet is ascii")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotificationRefs {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ticket_id: Option<TicketId>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub child_session_id: Option<SessionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: NotificationId,
    pub severity: Severity,
    pub subject: String,
    pub body: String,
    pub refs: NotificationRefs,
    pub emitted_by: SessionId,
    pub emitted_at: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn severity_ordering() {
        assert!(Severity::Blocker > Severity::Error);
        assert!(Severity::Error > Severity::Warn);
        assert!(Severity::Warn > Severity::Info);
    }

    #[test]
    fn severity_serde_lowercase() {
        assert_eq!(serde_json::to_string(&Severity::Warn).unwrap(), "\"warn\"");
        assert_eq!(serde_json::to_string(&Severity::Blocker).unwrap(), "\"blocker\"");
        let parsed: Severity = serde_json::from_str("\"blocker\"").unwrap();
        assert_eq!(parsed, Severity::Blocker);
    }

    #[test]
    fn notification_id_sequential() {
        let idgen = NotificationIdGenerator::new();
        let a = idgen.next();
        let b = idgen.next();
        assert_ne!(a, b);
        assert!(a.0.starts_with("n_"));
        assert!(b.0.starts_with("n_"));
    }

    #[test]
    fn notification_id_collision_free_under_concurrency() {
        use std::collections::HashSet;
        use std::thread;
        let idgen = Arc::new(NotificationIdGenerator::new());
        let n_threads = 8;
        let per_thread = 1000;
        let mut handles = Vec::new();
        for _ in 0..n_threads {
            let g = idgen.clone();
            handles.push(thread::spawn(move || {
                (0..per_thread).map(|_| g.next()).collect::<Vec<_>>()
            }));
        }
        let mut all = HashSet::new();
        for h in handles {
            for id in h.join().unwrap() {
                assert!(all.insert(id), "duplicate id under concurrency");
            }
        }
        assert_eq!(all.len(), n_threads * per_thread);
    }

    #[test]
    fn notification_serde_roundtrip() {
        let n = Notification {
            id: NotificationId("n_1".into()),
            severity: Severity::Error,
            subject: "subj".into(),
            body: "body".into(),
            refs: NotificationRefs::default(),
            emitted_by: SessionId::for_test("s_1"),
            emitted_at: SystemTime::UNIX_EPOCH,
        };
        let json = serde_json::to_string(&n).unwrap();
        let back: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, n.id);
        assert_eq!(back.severity, n.severity);
        assert_eq!(back.subject, n.subject);
    }

    #[test]
    fn notification_refs_optional_fields_omitted_when_none() {
        let r = NotificationRefs::default();
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "{}");
    }
}
