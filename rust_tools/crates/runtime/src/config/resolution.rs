use std::time::{Duration, Instant};

use crate::agents::AgentConfig;
use crate::config::approvals_section::ApprovalsConfig;
use crate::config::runtime_section::RuntimeConfig;

pub const HARD_APPROVAL_TTL_FALLBACK: Duration = Duration::from_secs(600);

pub struct SessionDeadline {
    pub deadline: Option<Instant>,
}

pub fn resolve_max_turns(agent: &AgentConfig, runtime: &RuntimeConfig) -> u32 {
    agent.max_turns.unwrap_or(runtime.default_max_turns).max(1)
}

pub fn resolve_session_ttl(agent: &AgentConfig, runtime: &RuntimeConfig) -> Option<Duration> {
    agent.session_ttl.or(runtime.default_session_ttl)
}

pub fn resolve_child_ttl(
    now: Instant,
    parent: SessionDeadline,
    runtime: &RuntimeConfig,
) -> Instant {
    let default = now + runtime.default_child_ttl;
    let Some(deadline) = parent.deadline else {
        return default;
    };
    let margin = runtime.child_ttl_margin;
    let remaining = deadline.saturating_duration_since(now);
    if remaining > margin + runtime.default_child_ttl {
        default
    } else if remaining > margin {
        deadline - margin
    } else {
        deadline
    }
}

pub fn resolve_approval_ttl(
    action_kind: &str,
    approvals: &ApprovalsConfig,
    runtime_default: Option<Duration>,
) -> Duration {
    if let Some(d) = approvals.ttl_overrides.get(action_kind).copied() {
        return d;
    }
    runtime_default.unwrap_or(HARD_APPROVAL_TTL_FALLBACK)
}

pub fn cap_ticket_expiry(
    now: Instant,
    action_ttl: Duration,
    session_deadline: Option<Instant>,
) -> Instant {
    let action_deadline = now + action_ttl;
    match session_deadline {
        Some(sd) if sd < action_deadline => sd,
        _ => action_deadline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuntimeConfig;

    fn agent(max_turns: Option<u32>, session_ttl: Option<Duration>) -> AgentConfig {
        AgentConfig {
            name: "test".into(),
            description: String::new(),
            preamble: String::new(),
            authz_mode: crate::agents::AuthzMode::Cedarling,
            permitted_actions: vec![],
            max_turns,
            session_ttl,
        }
    }

    #[test]
    fn resolve_max_turns_agent_some_wins() {
        let agent = agent(Some(99), None);
        let runtime = RuntimeConfig {
            default_max_turns: 40,
            ..Default::default()
        };
        assert_eq!(resolve_max_turns(&agent, &runtime), 99);
    }

    #[test]
    fn resolve_max_turns_runtime_default_when_agent_none() {
        let agent = agent(None, None);
        let runtime = RuntimeConfig {
            default_max_turns: 7,
            ..Default::default()
        };
        assert_eq!(resolve_max_turns(&agent, &runtime), 7);
    }

    #[test]
    fn resolve_max_turns_fallback_to_constant() {
        let agent = agent(None, None);
        let runtime = RuntimeConfig::default();
        assert_eq!(resolve_max_turns(&agent, &runtime), 40);
    }

    #[test]
    fn resolve_max_turns_min_clamped_to_1() {
        let agent = agent(Some(0), None);
        let runtime = RuntimeConfig::default();
        assert_eq!(resolve_max_turns(&agent, &runtime), 1);
    }

    #[test]
    fn resolve_session_ttl_agent_some_wins() {
        let agent = agent(None, Some(Duration::from_secs(300)));
        let runtime = RuntimeConfig {
            default_session_ttl: Some(Duration::from_secs(600)),
            ..Default::default()
        };
        assert_eq!(
            resolve_session_ttl(&agent, &runtime),
            Some(Duration::from_secs(300))
        );
    }

    #[test]
    fn resolve_session_ttl_runtime_default_when_agent_none() {
        let agent = agent(None, None);
        let runtime = RuntimeConfig {
            default_session_ttl: Some(Duration::from_secs(600)),
            ..Default::default()
        };
        assert_eq!(
            resolve_session_ttl(&agent, &runtime),
            Some(Duration::from_secs(600))
        );
    }

    #[test]
    fn resolve_session_ttl_both_none_returns_none() {
        let agent = agent(None, None);
        let runtime = RuntimeConfig::default();
        assert_eq!(resolve_session_ttl(&agent, &runtime), None);
    }

    #[test]
    fn resolve_child_ttl_no_parent_deadline() {
        let now = Instant::now();
        let runtime = RuntimeConfig::default();
        let result = resolve_child_ttl(now, SessionDeadline { deadline: None }, &runtime);
        assert_eq!(result, now + runtime.default_child_ttl);
    }

    #[test]
    fn resolve_child_ttl_plenty_remaining() {
        let now = Instant::now();
        let parent_deadline = now + Duration::from_secs(3600);
        let runtime = RuntimeConfig::default();
        let result = resolve_child_ttl(
            now,
            SessionDeadline {
                deadline: Some(parent_deadline),
            },
            &runtime,
        );
        assert_eq!(result, now + runtime.default_child_ttl);
    }

    #[test]
    fn resolve_child_ttl_tight_remaining() {
        let now = Instant::now();
        let parent_deadline = now + Duration::from_secs(120);
        let runtime = RuntimeConfig::default();
        let result = resolve_child_ttl(
            now,
            SessionDeadline {
                deadline: Some(parent_deadline),
            },
            &runtime,
        );
        assert_eq!(result, parent_deadline - runtime.child_ttl_margin);
    }

    #[test]
    fn resolve_child_ttl_very_tight_clamps_to_deadline() {
        let now = Instant::now();
        let parent_deadline = now + Duration::from_secs(10);
        let runtime = RuntimeConfig::default();
        let result = resolve_child_ttl(
            now,
            SessionDeadline {
                deadline: Some(parent_deadline),
            },
            &runtime,
        );
        assert_eq!(result, parent_deadline);
    }

    #[test]
    fn resolve_approval_ttl_override_wins() {
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("tool.test".to_string(), Duration::from_secs(120));
        let approvals = ApprovalsConfig {
            ttl_overrides: overrides,
        };
        let result = resolve_approval_ttl("tool.test", &approvals, Some(Duration::from_secs(600)));
        assert_eq!(result, Duration::from_secs(120));
    }

    #[test]
    fn resolve_approval_ttl_runtime_default_when_no_override() {
        let approvals = ApprovalsConfig::default();
        let result = resolve_approval_ttl("tool.test", &approvals, Some(Duration::from_secs(600)));
        assert_eq!(result, Duration::from_secs(600));
    }

    #[test]
    fn resolve_approval_ttl_hard_fallback_when_both_absent() {
        let approvals = ApprovalsConfig::default();
        let result = resolve_approval_ttl("tool.test", &approvals, None);
        assert_eq!(result, HARD_APPROVAL_TTL_FALLBACK);
    }

    #[test]
    fn resolve_approval_ttl_zero_is_valid_explicit_not_overridden() {
        let approvals = ApprovalsConfig::default();
        let result = resolve_approval_ttl("tool.test", &approvals, Some(Duration::ZERO));
        assert_eq!(result, Duration::ZERO);
    }

    #[test]
    fn cap_ticket_expiry_session_deadline_sooner() {
        let now = Instant::now();
        let session_deadline = Some(now + Duration::from_secs(60));
        let action_ttl = Duration::from_secs(600);
        let result = cap_ticket_expiry(now, action_ttl, session_deadline);
        assert_eq!(result, now + Duration::from_secs(60));
    }

    #[test]
    fn cap_ticket_expiry_action_ttl_sooner() {
        let now = Instant::now();
        let session_deadline = Some(now + Duration::from_secs(3600));
        let action_ttl = Duration::from_secs(60);
        let result = cap_ticket_expiry(now, action_ttl, session_deadline);
        assert_eq!(result, now + Duration::from_secs(60));
    }

    #[test]
    fn cap_ticket_expiry_no_session_deadline() {
        let now = Instant::now();
        let action_ttl = Duration::from_secs(60);
        let result = cap_ticket_expiry(now, action_ttl, None);
        assert_eq!(result, now + Duration::from_secs(60));
    }
}
