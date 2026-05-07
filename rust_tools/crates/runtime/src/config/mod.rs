use std::collections::HashMap;

use serde::Deserialize;

use crate::agents::AgentConfig;

mod approvals_section;
mod resolution;
mod runtime_section;
mod validation;

pub use approvals_section::ApprovalsConfig;
pub use resolution::*;
pub use runtime_section::{PersistenceBackend, RuntimeConfig};
pub use validation::{ConfigError, validate};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub approvals: ApprovalsConfig,
    pub orchestrator: AgentConfig,
    pub agents: HashMap<String, AgentConfig>,
}

impl Config {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, ConfigError> {
        let cfg: Self = serde_yaml::from_str(yaml)?;
        validate(&cfg)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn full_yaml() -> &'static str {
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
  preamble: "test preamble"
  max_turns: 80

agents:
  researcher:
    name: researcher
    preamble: "researcher preamble"
    session_ttl: 15m
  verifier:
    name: verifier
    preamble: "verifier preamble"
"#
    }

    #[test]
    fn loads_full_config() {
        let cfg: Config = serde_yaml::from_str(full_yaml()).unwrap();
        assert_eq!(cfg.runtime.default_max_turns, 80);
        assert_eq!(cfg.approvals.ttl_overrides.len(), 1);
        assert_eq!(cfg.orchestrator.name, "orchestrator");
        assert_eq!(cfg.orchestrator.max_turns, Some(80));
        assert_eq!(cfg.agents.len(), 2);
        assert_eq!(
            cfg.agents["researcher"].session_ttl,
            Some(Duration::from_secs(15 * 60))
        );
    }

    #[test]
    fn loads_minimal_config() {
        let yaml = r#"
orchestrator:
  name: orchestrator
  preamble: "test"
agents: {}
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.runtime, RuntimeConfig::default());
        assert_eq!(cfg.approvals, ApprovalsConfig::default());
    }
}
