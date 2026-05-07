use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

mod option_map_humantime {
    use std::collections::HashMap;
    use std::time::Duration;

    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<HashMap<String, Duration>, D::Error> {
        let raw = HashMap::<String, String>::deserialize(d)?;
        raw.into_iter()
            .map(|(k, v)| {
                humantime::parse_duration(&v)
                    .map(|d| (k, d))
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ApprovalsConfig {
    #[serde(with = "option_map_humantime", default)]
    pub ttl_overrides: HashMap<String, Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ttl_overrides() {
        let yaml = r#"
ttl_overrides:
  "tool.doc_publish_live": 30m
  "tool.something": 5m
"#;
        let cfg: ApprovalsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            cfg.ttl_overrides.get("tool.doc_publish_live"),
            Some(&Duration::from_secs(30 * 60))
        );
        assert_eq!(
            cfg.ttl_overrides.get("tool.something"),
            Some(&Duration::from_secs(5 * 60))
        );
    }

    #[test]
    fn defaults_to_empty_overrides() {
        let cfg: ApprovalsConfig = serde_yaml::from_str("{}").unwrap();
        assert!(cfg.ttl_overrides.is_empty());
    }

    #[test]
    fn unknown_key_keeps_string_action_kind() {
        let yaml = r#"
ttl_overrides:
  "custom_action_123": 10m
"#;
        let cfg: ApprovalsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            cfg.ttl_overrides.get("custom_action_123"),
            Some(&Duration::from_secs(10 * 60))
        );
    }

    #[test]
    fn option_map_humantime_deserialize_parses_durations() {
        let yaml = r#"
a: 1m
b: 2h
"#;
        let map: HashMap<String, Duration> =
            option_map_humantime::deserialize(serde_yaml::Deserializer::from_str(yaml)).unwrap();
        assert_eq!(map.get("a"), Some(&Duration::from_secs(60)));
        assert_eq!(map.get("b"), Some(&Duration::from_secs(2 * 3600)));
    }
}
