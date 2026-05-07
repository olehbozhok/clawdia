//! HMAC-SHA256 binding of `(ticket_id, args_hash, decision)`. CLI/TUI path
//! signs with the local key; the JWT path receives a pre-signed value from a
//! browser-side approver.

use crate::approvals::outcome::ApprovalOutcome;
use crate::approvals::types::TicketId;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum SignError {
    #[error("invalid hmac key length")]
    KeyLen,
    #[error("signature verification failed")]
    VerifyFailed,
}

fn decision_byte(o: &ApprovalOutcome) -> u8 {
    match o {
        ApprovalOutcome::Approved => 1,
        ApprovalOutcome::Denied { .. } => 2,
        ApprovalOutcome::Expired => 3,
    }
}

fn mac_input(id: &TicketId, args_hash: &str, outcome: &ApprovalOutcome) -> Vec<u8> {
    let mut v = Vec::with_capacity(id.0.len() + args_hash.len() + 2);
    v.extend_from_slice(id.0.as_bytes());
    v.push(0);
    v.extend_from_slice(args_hash.as_bytes());
    v.push(decision_byte(outcome));
    v
}

pub fn sign(
    key: &[u8],
    id: &TicketId,
    args_hash: &str,
    outcome: &ApprovalOutcome,
) -> Result<Vec<u8>, SignError> {
    let mut m = HmacSha256::new_from_slice(key).map_err(|_| SignError::KeyLen)?;
    m.update(&mac_input(id, args_hash, outcome));
    Ok(m.finalize().into_bytes().to_vec())
}

pub fn verify(
    key: &[u8],
    id: &TicketId,
    args_hash: &str,
    outcome: &ApprovalOutcome,
    sig: &[u8],
) -> Result<(), SignError> {
    let mut m = HmacSha256::new_from_slice(key).map_err(|_| SignError::KeyLen)?;
    m.update(&mac_input(id, args_hash, outcome));
    m.verify_slice(sig).map_err(|_| SignError::VerifyFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Vec<u8> {
        vec![0xABu8; 32]
    }

    fn tid() -> TicketId {
        TicketId("tk_1".into())
    }

    #[test]
    fn sign_and_verify_ok() {
        let k = key();
        let sig = sign(&k, &tid(), "sha256:aa", &ApprovalOutcome::Approved).unwrap();
        verify(&k, &tid(), "sha256:aa", &ApprovalOutcome::Approved, &sig).unwrap();
    }

    #[test]
    fn tampered_ticket_id_fails() {
        let k = key();
        let sig = sign(&k, &tid(), "sha256:aa", &ApprovalOutcome::Approved).unwrap();
        let other = TicketId("tk_2".into());
        assert!(verify(&k, &other, "sha256:aa", &ApprovalOutcome::Approved, &sig).is_err());
    }

    #[test]
    fn tampered_args_hash_fails() {
        let k = key();
        let sig = sign(&k, &tid(), "sha256:aa", &ApprovalOutcome::Approved).unwrap();
        assert!(verify(&k, &tid(), "sha256:bb", &ApprovalOutcome::Approved, &sig).is_err());
    }

    #[test]
    fn flipped_decision_byte_fails() {
        let k = key();
        let sig = sign(&k, &tid(), "sha256:aa", &ApprovalOutcome::Approved).unwrap();
        let denied = ApprovalOutcome::Denied {
            reason: "no".into(),
        };
        assert!(verify(&k, &tid(), "sha256:aa", &denied, &sig).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let k = key();
        let sig = sign(&k, &tid(), "sha256:aa", &ApprovalOutcome::Approved).unwrap();
        let other = vec![0x11u8; 32];
        assert!(
            verify(
                &other,
                &tid(),
                "sha256:aa",
                &ApprovalOutcome::Approved,
                &sig
            )
            .is_err()
        );
    }
}
