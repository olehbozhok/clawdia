//! Approval ticket TTL resolution.
//!
//! Override > default > fallback (600s). Capped by session deadline so a ticket
//! never outlives the session that owns it.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const FALLBACK_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Default)]
pub struct TtlConfig {
    pub default_ttl: Option<Duration>,
    pub overrides: HashMap<String, Duration>,
}

pub fn resolve_ttl(cfg: &TtlConfig, action_kind: &str) -> Duration {
    cfg.overrides
        .get(action_kind)
        .copied()
        .or(cfg.default_ttl)
        .unwrap_or(FALLBACK_TTL)
}

pub fn cap_by_session_deadline(now: Instant, ttl: Duration, deadline: Option<Instant>) -> Duration {
    match deadline {
        Some(d) if d > now => ttl.min(d.saturating_duration_since(now)),
        Some(_) => Duration::ZERO,
        None => ttl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let mut cfg = TtlConfig {
            default_ttl: Some(Duration::from_secs(600)),
            overrides: HashMap::new(),
        };
        cfg.overrides
            .insert("act".into(), Duration::from_secs(1800));
        assert_eq!(resolve_ttl(&cfg, "act"), Duration::from_secs(1800));
    }

    #[test]
    fn default_when_no_override() {
        let cfg = TtlConfig {
            default_ttl: Some(Duration::from_secs(600)),
            overrides: HashMap::new(),
        };
        assert_eq!(resolve_ttl(&cfg, "act"), Duration::from_secs(600));
    }

    #[test]
    fn fallback_when_neither() {
        let cfg = TtlConfig::default();
        assert_eq!(resolve_ttl(&cfg, "act"), FALLBACK_TTL);
    }

    #[test]
    fn capped_by_session_deadline() {
        let now = Instant::now();
        let dl = now + Duration::from_secs(300);
        let capped = cap_by_session_deadline(now, Duration::from_secs(1800), Some(dl));
        assert!(capped <= Duration::from_secs(300));
        assert!(capped >= Duration::from_secs(299));
    }

    #[test]
    fn no_deadline_passthrough() {
        let now = Instant::now();
        assert_eq!(
            cap_by_session_deadline(now, Duration::from_secs(60), None),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn past_deadline_zero() {
        let now = Instant::now();
        let past = now - Duration::from_secs(1);
        assert_eq!(
            cap_by_session_deadline(now, Duration::from_secs(60), Some(past)),
            Duration::ZERO
        );
    }
}
