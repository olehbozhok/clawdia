//! Idempotence key for `approval_request`: same `(session, action, args)` →
//! same ticket. JCS-canonical args hash feeds a second sha256 with NUL-separated
//! components so distinct `(session, action)` tuples cannot collide.

use crate::approvals::canonical::{CanonicalError, canonical_hash};
use crate::sessions::SessionId;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn correlation_key(
    session: &SessionId,
    action_kind: &str,
    args: &Value,
) -> Result<String, CanonicalError> {
    let args_h = canonical_hash(args)?;
    let mut h = Sha256::new();
    h.update(session.as_str().as_bytes());
    h.update([0u8]);
    h.update(action_kind.as_bytes());
    h.update([0u8]);
    h.update(args_h.as_bytes());
    Ok(format!("sha256:{}", hex::encode(h.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sid(s: &str) -> SessionId {
        SessionId::for_test(s)
    }

    #[test]
    fn same_inputs_same_key() {
        let a = correlation_key(&sid("s1"), "act", &json!({"x": 1})).unwrap();
        let b = correlation_key(&sid("s1"), "act", &json!({"x": 1})).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_session_different_key() {
        let a = correlation_key(&sid("s1"), "act", &json!({"x": 1})).unwrap();
        let b = correlation_key(&sid("s2"), "act", &json!({"x": 1})).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn different_action_different_key() {
        let a = correlation_key(&sid("s1"), "act", &json!({"x": 1})).unwrap();
        let b = correlation_key(&sid("s1"), "other", &json!({"x": 1})).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn key_order_in_args_does_not_matter() {
        let a = correlation_key(&sid("s1"), "act", &json!({"a": 1, "b": 2})).unwrap();
        let b = correlation_key(&sid("s1"), "act", &json!({"b": 2, "a": 1})).unwrap();
        assert_eq!(a, b);
    }
}
