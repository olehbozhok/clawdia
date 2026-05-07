use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersistenceBackend {
    Memory,
    Sqlite,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub default_max_turns: u32,
    #[serde(with = "humantime_serde")]
    pub default_session_ttl: Option<Duration>,
    #[serde(with = "humantime_serde")]
    pub default_child_ttl: Duration,
    #[serde(with = "humantime_serde")]
    pub child_ttl_margin: Duration,
    #[serde(default, with = "humantime_serde")]
    pub approval_default_ttl: Option<Duration>,
    pub persistence: PersistenceBackend,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_max_turns: 40,
            default_session_ttl: None,
            default_child_ttl: Duration::from_secs(30 * 60),
            child_ttl_margin: Duration::from_secs(30),
            approval_default_ttl: None,
            persistence: PersistenceBackend::Memory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_yaml() -> &'static str {
        r#"
default_max_turns: 80
default_session_ttl: 2h
default_child_ttl: 15m
child_ttl_margin: 1m
approval_default_ttl: 5m
persistence: memory
"#
    }

    #[test]
    fn parses_full_runtime_section() {
        let cfg: RuntimeConfig = serde_yaml::from_str(full_yaml()).unwrap();
        assert_eq!(cfg.default_max_turns, 80);
        assert_eq!(cfg.default_session_ttl, Some(Duration::from_secs(2 * 3600)));
        assert_eq!(cfg.default_child_ttl, Duration::from_secs(15 * 60));
        assert_eq!(cfg.child_ttl_margin, Duration::from_secs(60));
        assert_eq!(cfg.approval_default_ttl, Some(Duration::from_secs(5 * 60)));
        assert_eq!(cfg.persistence, PersistenceBackend::Memory);
    }

    #[test]
    fn applies_defaults_when_section_missing() {
        let cfg: RuntimeConfig = serde_yaml::from_str("{}").unwrap();
        assert_eq!(cfg, RuntimeConfig::default());
    }

    #[test]
    fn parses_sqlite_backend() {
        let yaml = "persistence: sqlite\n";
        let cfg: RuntimeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.persistence, PersistenceBackend::Sqlite);
    }
}
