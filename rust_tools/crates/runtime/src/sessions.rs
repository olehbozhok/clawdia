//! Short, collision-safe sequential session identifiers.
//!
//! Memory note `feedback_unique_ids.md`: LLMs confuse UUIDs. Use short
//! sequential IDs but verify no collision within the generator's lifetime.

use std::sync::atomic::{AtomicU64, Ordering};

/// Opaque session identifier. Always rendered as `s_<base36 counter>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    /// Construct a `SessionId` from a previously-generated string. Validates
    /// the `s_` prefix to defend against accidental misuse (e.g. passing an
    /// arbitrary user string as a session id).
    pub fn from_string(raw: String) -> Result<Self, InvalidSessionId> {
        if !raw.starts_with("s_") || raw.len() < 3 {
            return Err(InvalidSessionId(raw));
        }
        Ok(Self(raw))
    }

    /// Test-only constructor that bypasses prefix validation. Used by Plan 03+
    /// tests for ergonomic short ids. (Decision D16.)
    #[cfg(test)]
    pub fn for_test(raw: &str) -> Self {
        Self(raw.to_string())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid session id: {0:?}")]
pub struct InvalidSessionId(pub String);

/// Sequential `SessionId` generator. Single-process, lock-free.
#[derive(Debug, Default)]
pub struct SessionIdGenerator {
    counter: AtomicU64,
}

impl SessionIdGenerator {
    pub fn new() -> Self {
        Self { counter: AtomicU64::new(0) }
    }

    pub fn next(&self) -> SessionId {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        SessionId(format!("s_{}", to_base36(n)))
    }
}

fn to_base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = Vec::with_capacity(13);
    while n > 0 {
        buf.push(ALPHABET[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).expect("base36 alphabet is ascii")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_produces_distinct_sequential_ids() {
        let gen_ = SessionIdGenerator::new();
        let a = gen_.next();
        let b = gen_.next();
        let c = gen_.next();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert!(a.as_str().starts_with("s_"));
    }

    #[test]
    fn generator_no_collisions_under_concurrency() {
        use std::sync::Arc;
        use std::thread;
        let gen_ = Arc::new(SessionIdGenerator::new());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let g = gen_.clone();
            handles.push(thread::spawn(move || {
                (0..1000).map(|_| g.next()).collect::<Vec<_>>()
            }));
        }
        let mut all = Vec::new();
        for h in handles {
            all.extend(h.join().unwrap());
        }
        let unique: std::collections::HashSet<_> = all.iter().cloned().collect();
        assert_eq!(unique.len(), all.len(), "collision detected");
    }

    #[test]
    fn from_string_accepts_well_formed() {
        let id = SessionId::from_string("s_1a".to_string()).unwrap();
        assert_eq!(id.as_str(), "s_1a");
    }

    #[test]
    fn from_string_rejects_missing_prefix() {
        assert!(SessionId::from_string("1a".to_string()).is_err());
    }
}
