use std::time::{Duration, Instant};

use runtime::config::{
    ApprovalsConfig, Config, HARD_APPROVAL_TTL_FALLBACK, PersistenceBackend, RuntimeConfig,
    cap_ticket_expiry, resolve_approval_ttl, resolve_max_turns, resolve_session_ttl, validate,
};

fn fixture_yaml() -> &'static str {
    r#"
runtime:
  default_max_turns: 80
  default_session_ttl: 2h
  default_child_ttl: 15m
  child_ttl_margin: 1m
  approval_default_ttl: 5m
  persistence: memory

approvals:
  ttl_overrides:
    "tool.doc_publish_live": 30m

orchestrator:
  name: orchestrator
  preamble: "You are the orchestrator"
  max_turns: 80

agents:
  researcher:
    name: researcher
    preamble: "You are a researcher"
    session_ttl: 15m
  verifier:
    name: verifier
    preamble: "You are a verifier"
"#
}

fn minimal_yaml() -> &'static str {
    r#"
orchestrator:
  name: orchestrator
  preamble: "test"
agents: {}
"#
}

#[test]
fn full_config_round_trip() {
    let cfg = Config::from_yaml_str(fixture_yaml()).unwrap();
    validate(&cfg).unwrap();

    assert_eq!(cfg.runtime.default_max_turns, 80);
    assert_eq!(
        cfg.runtime.default_session_ttl,
        Some(Duration::from_secs(2 * 3600))
    );
    assert_eq!(cfg.runtime.default_child_ttl, Duration::from_secs(15 * 60));
    assert_eq!(cfg.runtime.child_ttl_margin, Duration::from_secs(60));
    assert_eq!(
        cfg.runtime.approval_default_ttl,
        Some(Duration::from_secs(5 * 60))
    );
    assert_eq!(cfg.runtime.persistence, PersistenceBackend::Memory);

    assert_eq!(cfg.approvals.ttl_overrides.len(), 1);
    assert_eq!(
        cfg.approvals.ttl_overrides.get("tool.doc_publish_live"),
        Some(&Duration::from_secs(30 * 60))
    );

    assert_eq!(cfg.orchestrator.name, "orchestrator");
    assert_eq!(cfg.orchestrator.max_turns, Some(80));
    assert_eq!(cfg.agents.len(), 2);
    assert_eq!(
        cfg.agents["researcher"].session_ttl,
        Some(Duration::from_secs(15 * 60))
    );
    assert_eq!(cfg.agents["verifier"].session_ttl, None);
}

#[test]
fn full_config_resolvers() {
    let cfg = Config::from_yaml_str(fixture_yaml()).unwrap();

    // orchestrator max_turns: agent has Some(80), so that wins
    assert_eq!(resolve_max_turns(&cfg.orchestrator, &cfg.runtime), 80);

    // researcher max_turns: no override, falls back to runtime default (80)
    assert_eq!(
        resolve_max_turns(&cfg.agents["researcher"], &cfg.runtime),
        80
    );

    // verifier max_turns: no override, falls back to runtime default (80)
    assert_eq!(resolve_max_turns(&cfg.agents["verifier"], &cfg.runtime), 80);

    // researcher session_ttl: agent has Some(15m)
    assert_eq!(
        resolve_session_ttl(&cfg.agents["researcher"], &cfg.runtime),
        Some(Duration::from_secs(15 * 60))
    );

    // verifier session_ttl: agent has None, runtime has Some(2h)
    assert_eq!(
        resolve_session_ttl(&cfg.agents["verifier"], &cfg.runtime),
        Some(Duration::from_secs(2 * 3600))
    );

    // approval TTL for tool.doc_publish_live: override wins (30m)
    assert_eq!(
        resolve_approval_ttl(
            "tool.doc_publish_live",
            &cfg.approvals,
            cfg.runtime.approval_default_ttl,
        ),
        Duration::from_secs(30 * 60)
    );

    // approval TTL for unknown tool: runtime default (5m)
    assert_eq!(
        resolve_approval_ttl(
            "tool.unknown",
            &cfg.approvals,
            cfg.runtime.approval_default_ttl
        ),
        Duration::from_secs(5 * 60)
    );

    // cap_ticket_expiry: session deadline (now + 5m) < action ttl (30m) => deadline wins
    let now = Instant::now();
    let session_deadline = Some(now + Duration::from_secs(5 * 60));
    let capped = cap_ticket_expiry(now, Duration::from_secs(30 * 60), session_deadline);
    assert_eq!(capped, now + Duration::from_secs(5 * 60));
}

#[test]
fn minimal_config_uses_defaults() {
    let cfg = Config::from_yaml_str(minimal_yaml()).unwrap();
    validate(&cfg).unwrap();

    assert_eq!(cfg.runtime, RuntimeConfig::default());
    assert_eq!(cfg.approvals, ApprovalsConfig::default());

    // All resolvers return documented defaults
    assert_eq!(resolve_max_turns(&cfg.orchestrator, &cfg.runtime), 40);
    assert_eq!(resolve_session_ttl(&cfg.orchestrator, &cfg.runtime), None);
    assert_eq!(
        resolve_approval_ttl(
            "any.action",
            &cfg.approvals,
            cfg.runtime.approval_default_ttl
        ),
        HARD_APPROVAL_TTL_FALLBACK,
    );
}

#[test]
fn repo_agents_yaml_loads() {
    let yaml = include_str!("../../../config/agents.yaml");
    let cfg = Config::from_yaml_str(yaml).expect("agents.yaml must parse and validate");
    validate(&cfg).unwrap();

    // Default runtime values from agents.yaml
    assert_eq!(cfg.runtime.default_max_turns, 40);

    // Orchestrator override: max_turns: 80
    assert_eq!(resolve_max_turns(&cfg.orchestrator, &cfg.runtime), 80);

    // Sub-agents use runtime default
    for agent in cfg.agents.values() {
        assert_eq!(
            resolve_max_turns(agent, &cfg.runtime),
            40,
            "sub-agent {} should use runtime default",
            agent.name,
        );
    }
}
