use crate::config::Config;
use crate::config::PersistenceBackend;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("persistence backend `{backend}` is not implemented in v1; use `memory`")]
    PersistenceUnimplemented { backend: String },
    #[error("serde error: {0}")]
    Serde(#[from] serde_yaml::Error),
}

pub fn validate(cfg: &Config) -> Result<(), ConfigError> {
    if cfg.runtime.persistence == PersistenceBackend::Sqlite {
        return Err(ConfigError::PersistenceUnimplemented {
            backend: "sqlite".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentConfig;
    use crate::config::{ApprovalsConfig, RuntimeConfig};
    use std::collections::HashMap;

    fn make_config(persistence: PersistenceBackend) -> Config {
        Config {
            runtime: RuntimeConfig {
                persistence,
                ..Default::default()
            },
            approvals: ApprovalsConfig::default(),
            orchestrator: AgentConfig {
                name: "orchestrator".into(),
                description: String::new(),
                preamble: "test".into(),
                authz_mode: crate::agents::AuthzMode::Cedarling,
                permitted_actions: vec![],
                max_turns: None,
                session_ttl: None,
            },
            agents: HashMap::new(),
        }
    }

    #[test]
    fn rejects_sqlite_persistence() {
        let cfg = make_config(PersistenceBackend::Sqlite);
        let err = validate(&cfg).unwrap_err();
        assert!(
            matches!(&err, ConfigError::PersistenceUnimplemented { backend } if backend == "sqlite")
        );
    }

    #[test]
    fn accepts_memory_persistence() {
        let cfg = make_config(PersistenceBackend::Memory);
        assert!(validate(&cfg).is_ok());
    }
}
