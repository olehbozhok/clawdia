//! CedarAuthz — wraps Cedarling's `authorize_unsigned` API for agent tool authorization.
//!
//! Loads a Cedar policy store from a directory and authorizes agent tool calls
//! against the loaded policies. Action = tool name directly; resource is always
//! the static `AgentPolicy::System::"clawdia"` entity.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use cedarling::{
    AuthorizationConfig, BootstrapConfig, CedarEntityMapping, Cedarling, DataStoreConfig,
    EntityBuilderConfig, EntityData, JsonRule, JwtConfig, LogConfig, LogLevel, LogTypeConfig,
    MemoryLogConfig, PolicyStoreConfig, PolicyStoreSource, RequestUnsigned,
};
use serde_json::{Value, json};

use tools::authz::{AuthorizationDecision, AuthorizationResult};

/// Wraps a Cedarling instance for agent authorization.
pub struct CedarAuthz {
    cedarling: Cedarling,
    system_entity_id: String,
}

impl CedarAuthz {
    /// Load a CedarAuthz from a policy store directory.
    ///
    /// The directory must contain `metadata.json`, `schema.cedarschema`, `policies/`, and `entities/`.
    /// The System entity ID is read from `entities/system.json`.
    pub async fn from_directory(path: &Path) -> Result<Self> {
        let system_entity_id = read_system_entity_id(path)
            .context("reading System entity ID from entities/system.json")?;

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
            system_entity_id,
        })
    }

    /// Authorize a tool call for a given agent.
    ///
    /// Returns `Allow` if Cedar policies permit the action, `Deny` otherwise.
    /// If the tool name does not match any action in the Cedar schema, Cedar
    /// will deny it (deny by default).
    pub async fn authorize(
        &self,
        agent_name: &str,
        tool_name: &str,
        args: &Value,
    ) -> AuthorizationResult {
        let cedar_action = format!("AgentPolicy::Action::\"{tool_name}\"");

        let context = build_context(tool_name, args);

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
                    entity_type: "AgentPolicy::System".to_string(),
                    id: self.system_entity_id.clone(),
                },
                attributes: HashMap::new(),
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
                        reason: Some(format!("Cedar policy denied {agent_name} -> {tool_name}")),
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

/// Read the System entity ID from `entities/system.json`.
///
/// Expects a JSON array with at least one entity whose `uid.type` is `AgentPolicy::System`.
fn read_system_entity_id(policy_store_dir: &Path) -> Result<String> {
    let path = policy_store_dir.join("entities/system.json");
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let entities: Vec<Value> =
        serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;

    for entity in &entities {
        let entity_type = entity
            .pointer("/uid/type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if entity_type == "AgentPolicy::System" {
            if let Some(id) = entity.pointer("/uid/id").and_then(|v| v.as_str()) {
                return Ok(id.to_string());
            }
        }
    }

    anyhow::bail!("no AgentPolicy::System entity found in {}", path.display())
}

/// Build the Cedar context object from the tool name and tool arguments.
///
/// Context fields are determined by tool name:
/// - `fetch_content`, `chrome_get_web_content` → extract `requested_domain`
/// - `doc_*` (except doc_list, doc_status, doc_create, doc_assemble, doc_publish_*) → extract `campaign_id`
/// - Everything else → empty context
fn build_context(tool_name: &str, args: &Value) -> Value {
    let mut ctx = serde_json::Map::new();

    if tool_name == "fetch_content" || tool_name.starts_with("chrome_") {
        if let Some(domain) = extract_domain_from_value(args) {
            ctx.insert("requested_domain".to_string(), Value::String(domain));
        }
    } else if tool_name.starts_with("doc_") && needs_campaign_id(tool_name) {
        if let Some(id) = args.get("campaign_id").and_then(|v| v.as_str()) {
            ctx.insert("campaign_id".to_string(), Value::String(id.to_string()));
        }
    }

    Value::Object(ctx)
}

/// Returns true if a doc_* tool should include campaign_id in context.
fn needs_campaign_id(tool_name: &str) -> bool {
    !matches!(
        tool_name,
        "doc_list"
            | "doc_status"
            | "doc_create"
            | "doc_assemble"
            | "doc_publish_draft"
            | "doc_publish_live"
    )
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
    fn build_context_fetch_content_with_url() {
        let args = json!({"url": "https://oceana.org/reports/trawling"});
        let ctx = build_context("fetch_content", &args);
        assert_eq!(ctx["requested_domain"], "oceana.org");
    }

    #[test]
    fn build_context_chrome_with_url() {
        let args = json!({"url": "https://oceana.org/reports/trawling"});
        let ctx = build_context("chrome_get_web_content", &args);
        assert_eq!(ctx["requested_domain"], "oceana.org");
    }

    #[test]
    fn build_context_search_empty() {
        let args = json!({"query": "bottom trawling"});
        let ctx = build_context("search", &args);
        assert_eq!(ctx, json!({}));
    }

    #[test]
    fn build_context_doc_add_statement_with_id() {
        let args = json!({"campaign_id": "camp-1"});
        let ctx = build_context("doc_add_statement", &args);
        assert_eq!(ctx["campaign_id"], "camp-1");
    }

    #[test]
    fn build_context_doc_list_empty() {
        let args = json!({"campaign_id": "camp-1"});
        let ctx = build_context("doc_list", &args);
        assert_eq!(ctx, json!({}));
    }

    #[test]
    fn build_context_doc_create_empty() {
        let args = json!({"campaign_id": "camp-1"});
        let ctx = build_context("doc_create", &args);
        assert_eq!(ctx, json!({}));
    }

    // ── needs_campaign_id tests ──

    #[test]
    fn needs_campaign_id_for_write_tools() {
        assert!(needs_campaign_id("doc_add_statement"));
        assert!(needs_campaign_id("doc_set_verdict"));
        assert!(needs_campaign_id("doc_write_content"));
        assert!(needs_campaign_id("doc_get_verified"));
        assert!(needs_campaign_id("doc_get"));
        assert!(needs_campaign_id("doc_list_statements"));
    }

    #[test]
    fn no_campaign_id_for_manage_tools() {
        assert!(!needs_campaign_id("doc_list"));
        assert!(!needs_campaign_id("doc_status"));
        assert!(!needs_campaign_id("doc_create"));
        assert!(!needs_campaign_id("doc_assemble"));
        assert!(!needs_campaign_id("doc_publish_draft"));
        assert!(!needs_campaign_id("doc_publish_live"));
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
        assert_eq!(extract_domain_from_value(&v).as_deref(), Some("oceana.org"));
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
        let policy_dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/policies");
        let policy_dir = policy_dir
            .canonicalize()
            .expect("policy store directory should exist");

        let authz = CedarAuthz::from_directory(&policy_dir)
            .await
            .expect("should load policy store");

        // researcher allowed to search (action = tool name directly)
        let result = authz.authorize("researcher", "search", &json!({})).await;
        assert_eq!(
            result.decision,
            AuthorizationDecision::Allow,
            "researcher should be allowed to search: {:?}",
            result.reason
        );

        // researcher denied doc_create (no policy for researcher + doc_create)
        let result = authz
            .authorize("researcher", "doc_create", &json!({}))
            .await;
        assert_eq!(
            result.decision,
            AuthorizationDecision::Deny,
            "researcher should be denied doc_create"
        );

        // unknown tool denied (action not in schema → Cedar denies)
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
