//! RFC 8785 JCS canonicalization + sha256 hash for approval ticket args.
//!
//! Floats rejected at boundary — `serde_jcs` would serialize them but
//! float→string is lossy across implementations. Approval args MUST hash
//! identically across processes; rejecting floats keeps that invariant tight.

use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CanonicalError {
    #[error("floats not allowed at path {path}")]
    FloatNotAllowed { path: String },
    #[error("jcs serialization failed: {0}")]
    Jcs(String),
}

pub fn canonical_hash(value: &Value) -> Result<String, CanonicalError> {
    reject_floats(value, &mut String::new())?;
    let bytes = serde_jcs::to_vec(value).map_err(|e| CanonicalError::Jcs(e.to_string()))?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("sha256:{}", hex::encode(h.finalize())))
}

fn reject_floats(v: &Value, path: &mut String) -> Result<(), CanonicalError> {
    match v {
        Value::Number(n)
            if n.as_f64().is_some() && n.as_i64().is_none() && n.as_u64().is_none() =>
        {
            Err(CanonicalError::FloatNotAllowed { path: path.clone() })
        }
        Value::Array(xs) => {
            for (i, x) in xs.iter().enumerate() {
                let len = path.len();
                use std::fmt::Write;
                let _ = write!(path, "[{i}]");
                reject_floats(x, path)?;
                path.truncate(len);
            }
            Ok(())
        }
        Value::Object(m) => {
            for (k, x) in m {
                let len = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(k);
                reject_floats(x, path)?;
                path.truncate(len);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_independent() {
        let a = canonical_hash(&json!({"b": 1, "a": 2})).unwrap();
        let b = canonical_hash(&json!({"a": 2, "b": 1})).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn floats_rejected() {
        let err = canonical_hash(&json!({"x": 1.5})).unwrap_err();
        match err {
            CanonicalError::FloatNotAllowed { path } => assert_eq!(path, "x"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn nested_floats_rejected() {
        let err = canonical_hash(&json!({"a": [{"b": 2.0}]})).unwrap_err();
        match err {
            CanonicalError::FloatNotAllowed { path } => assert_eq!(path, "a[0].b"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn null_preserved() {
        let with_null = canonical_hash(&json!({"a": null})).unwrap();
        let empty = canonical_hash(&json!({})).unwrap();
        assert_ne!(with_null, empty);
    }

    #[test]
    fn prefix_present() {
        let h = canonical_hash(&json!({"x": 1})).unwrap();
        assert!(h.starts_with("sha256:"));
        let hex = &h["sha256:".len()..];
        assert_eq!(hex.len(), 64);
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}
