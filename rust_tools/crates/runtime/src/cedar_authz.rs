//! CedarAuthz — wraps Cedarling's `authorize_unsigned` API for agent tool authorization.
//!
//! Loads a Cedar policy store from a directory and a `tool_action_map.json` file,
//! then authorizes agent tool calls against the loaded policies.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use cedarling::{
    AuthorizationConfig, BootstrapConfig, CedarEntityMapping, Cedarling, DataStoreConfig,
    EntityBuilderConfig, EntityData, JsonRule, JwtConfig, LogConfig, LogLevel, LogTypeConfig,
    MemoryLogConfig, PolicyStoreConfig, PolicyStoreSource, RequestUnsigned,
};
use serde_json::{json, Value};

use tools::authz::{AuthorizationDecision, AuthorizationResult};

/// Maps MCP tool names to Cedar action names.
type ToolActionMap = HashMap<String, String>;

/// Wraps a Cedarling instance and a tool-to-action mapping for agent authorization.
pub struct CedarAuthz {
    cedarling: Cedarling,
    tool_action_map: ToolActionMap,
}

impl CedarAuthz {
    /// Load a CedarAuthz from a policy store directory.
    ///
    /// The directory must contain `metadata.json`, `schema.cedarschema`, `policies/`, `entities/`,
    /// and a sibling or contained `tool_action_map.json`.
    pub async fn from_directory(path: &Path) -> Result<Self> {
        let tool_map_path = path.join("tool_action_map.json");
        let tool_map_bytes = std::fs::read_to_string(&tool_map_path)
            .with_context(|| format!("reading tool_action_map.json from {}", path.display()))?;
        let tool_action_map: ToolActionMap = serde_json::from_str(&tool_map_bytes)
            .context("parsing tool_action_map.json")?;

        let config = BootstrapConfig {
            application_name: "clawdia".to_string(),
            log_config: LogConfig {
                log_type: LogTypeConfig::Memory(MemoryLogConfig {
                    log_ttl: 60,
                    max_items: None,
                    max_item_size: None,
                }),
                log_level: LogLevel::INFO,
            },
            policy_store_config: PolicyStoreConfig {
                source: PolicyStoreSource::Directory(path.to_path_buf()),
            },
            jwt_config: JwtConfig::new_without_validation(),
            authorization_config: AuthorizationConfig {
                principal_bool_operator: JsonRule::new(
                    json!({"===": [{"var": "AgentPolicy::Agent"}, "ALLOW"]}),
                )
                .context("creating principal_bool_operator JsonRule")?,
                ..AuthorizationConfig::default()
            },
            entity_builder_config: EntityBuilderConfig::default(),
            lock_config: None,
            max_default_entities: None,
            max_base64_size: None,
            data_store_config: DataStoreConfig::default(),
        };

        let cedarling = Cedarling::new(&config)
            .await
            .context("initializing Cedarling from policy store directory")?;

        Ok(Self {
            cedarling,
            tool_action_map,
        })
    }

    /// Authorize a tool call for a given agent.
    ///
    /// Returns `Allow` if Cedar policies permit the action, `Deny` otherwise.
    /// If the tool is not in `tool_action_map.json`, the call is denied.
    pub async fn authorize(
        &self,
        agent_name: &str,
        tool_name: &str,
        args: &Value,
    ) -> AuthorizationResult {
        let Some(action_name) = self.tool_action_map.get(tool_name) else {
            return AuthorizationResult {
                decision: AuthorizationDecision::Deny,
                reason: Some(format!(
                    "tool '{tool_name}' not found in tool_action_map"
                )),
            };
        };

        let cedar_action = format!("AgentPolicy::Action::\"{action_name}\"");

        let context = build_context(action_name, args);

        let request = RequestUnsigned {
            principals: vec![EntityData {
                cedar_mapping: CedarEntityMapping {
                    entity_type: "AgentPolicy::Agent".to_string(),
                    id: agent_name.to_string(),
                },
                attributes: HashMap::from([(
                    "agent_type".to_string(),
                    Value::String(agent_name.to_string()),
                )]),
            }],
            action: cedar_action,
            resource: EntityData {
                cedar_mapping: CedarEntityMapping {
                    entity_type: "AgentPolicy::Tool".to_string(),
                    id: tool_name.to_string(),
                },
                attributes: HashMap::from([
                    ("tool_type".to_string(), Value::String("mcp".to_string())),
                    ("domain".to_string(), Value::String("unknown".to_string())),
                ]),
            },
            context,
        };

        match self.cedarling.authorize_unsigned(request).await {
            Ok(result) => {
                if result.decision {
                    AuthorizationResult {
                        decision: AuthorizationDecision::Allow,
                        reason: None,
                    }
                } else {
                    AuthorizationResult {
                        decision: AuthorizationDecision::Deny,
                        reason: Some(format!(
                            "Cedar policy denied {agent_name} -> {tool_name}"
                        )),
                    }
                }
            }
            Err(e) => AuthorizationResult {
                decision: AuthorizationDecision::Deny,
                reason: Some(format!("Cedarling authorization error: {e}")),
            },
        }
    }
}

/// Build the Cedar context object from the action name and tool arguments.
fn build_context(action_name: &str, args: &Value) -> Value {
    let mut ctx = serde_json::Map::new();

    match action_name {
        "web_fetch" => {
            if let Some(domain) = extract_domain_from_value(args) {
                ctx.insert("requested_domain".to_string(), Value::String(domain));
            }
        }
        "campaign_read" | "campaign_write" => {
            if let Some(id) = args
                .get("campaign_id")
                .and_then(|v| v.as_str())
            {
                ctx.insert("campaign_id".to_string(), Value::String(id.to_string()));
            }
        }
        // campaign_manage and others: empty context
        _ => {}
    }

    Value::Object(ctx)
}

/// Extract a domain from a Value that may contain a `url`, `domain`, or `uri` field.
fn extract_domain_from_value(v: &Value) -> Option<String> {
    let raw = v
        .get("url")
        .or_else(|| v.get("domain"))
        .or_else(|| v.get("uri"))
        .and_then(|val| val.as_str())?;

    // Try to parse as URL and extract host
    if let Some(after_scheme) = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
    {
        Some(after_scheme.split('/').next()?.to_string())
    } else {
        Some(raw.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_context tests ──

    #[test]
    fn build_context_web_fetch_with_url() {
        let args = json!({"url": "https://oceana.org/reports/trawling"});
        let ctx = build_context("web_fetch", &args);
        assert_eq!(ctx["requested_domain"], "oceana.org");
    }

    #[test]
    fn build_context_web_fetch_no_url() {
        let args = json!({"query": "bottom trawling"});
        let ctx = build_context("web_fetch", &args);
        assert_eq!(ctx, json!({}));
    }

    #[test]
    fn build_context_campaign_read_with_id() {
        let args = json!({"campaign_id": "camp-1"});
        let ctx = build_context("campaign_read", &args);
        assert_eq!(ctx["campaign_id"], "camp-1");
    }

    #[test]
    fn build_context_campaign_manage_empty() {
        let args = json!({"campaign_id": "camp-1"});
        let ctx = build_context("campaign_manage", &args);
        assert_eq!(ctx, json!({}));
    }

    // ── extract_domain tests ──

    #[test]
    fn extract_domain_from_https_url() {
        let v = json!({"url": "https://fisheries.noaa.gov/article/123"});
        assert_eq!(
            extract_domain_from_value(&v).as_deref(),
            Some("fisheries.noaa.gov")
        );
    }

    #[test]
    fn extract_domain_from_bare_domain() {
        let v = json!({"domain": "oceana.org"});
        assert_eq!(
            extract_domain_from_value(&v).as_deref(),
            Some("oceana.org")
        );
    }

    #[test]
    fn extract_domain_returns_none_when_missing() {
        let v = json!({"query": "trawling"});
        assert_eq!(extract_domain_from_value(&v), None);
    }

    // ── integration test ──

    #[tokio::test]
    async fn integration_policy_store_authorization() {
        // Path relative to the runtime crate root
        let policy_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/policies");
        let policy_dir = policy_dir
            .canonicalize()
            .expect("policy store directory should exist");

        let authz = CedarAuthz::from_directory(&policy_dir)
            .await
            .expect("should load policy store");

        // researcher allowed to search (mapped to web_fetch action)
        let result = authz
            .authorize("researcher", "search", &json!({}))
            .await;
        assert_eq!(
            result.decision,
            AuthorizationDecision::Allow,
            "researcher should be allowed to search: {:?}",
            result.reason
        );

        // researcher denied doc_create (mapped to campaign_manage action)
        let result = authz
            .authorize("researcher", "doc_create", &json!({}))
            .await;
        assert_eq!(
            result.decision,
            AuthorizationDecision::Deny,
            "researcher should be denied doc_create"
        );

        // unknown tool denied (not in tool_action_map)
        let result = authz
            .authorize("researcher", "rm_rf_slash", &json!({}))
            .await;
        assert_eq!(
            result.decision,
            AuthorizationDecision::Deny,
            "unknown tool should be denied"
        );
    }
}
