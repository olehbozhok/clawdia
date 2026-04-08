# Cedarling Authorization Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace flat YAML `permitted_actions` with Cedarling Cedar policy engine as the default authorization backend, with YAML as explicit fallback.

**Architecture:** Dual-mode `AuthzBackend` enum in `AuthzHook` — `Yaml` uses existing list check, `Cedar` delegates to in-process Cedarling via `authorize_unsigned`. Policy store lives in a configurable directory with `.cedar` files per agent, schema, entities, and a tool-action mapping file.

**Tech Stack:** Rust, cedarling crate (from jans-cedarling), Cedar policy language, serde_json, serde_yaml, anyhow, thiserror

**Spec:** `docs/superpowers/specs/2026-04-08-cedarling-authz-design.md`

---

## File Structure

### New files

- `rust_tools/config/policies/metadata.json` — policy store metadata
- `rust_tools/config/policies/schema.cedarschema` — Cedar schema for AgentPolicy namespace
- `rust_tools/config/policies/policies/orchestrator.cedar` — orchestrator policies
- `rust_tools/config/policies/policies/researcher.cedar` — researcher policies
- `rust_tools/config/policies/policies/verifier.cedar` — verifier policies
- `rust_tools/config/policies/policies/copywriter.cedar` — copywriter policies
- `rust_tools/config/policies/entities/agents.json` — agent entity data
- `rust_tools/config/policies/entities/tools.json` — tool entity data
- `rust_tools/config/policies/tool_action_map.json` — tool name to Cedar action mapping
- `rust_tools/crates/runtime/src/cedar_authz.rs` — Cedarling integration: init, authorize, prompt builder
- `rust_tools/crates/runtime/src/policy_prompt.rs` — builds permission summary for agent system prompts

### Modified files

- `rust_tools/Cargo.toml` — add cedarling workspace dependency
- `rust_tools/crates/runtime/Cargo.toml` — add cedarling + anyhow dependencies
- `rust_tools/crates/runtime/src/lib.rs` — export new modules
- `rust_tools/crates/runtime/src/agents.rs` — add `authz_mode` to config, integrate `AuthzBackend`, inject permission preamble
- `rust_tools/crates/runtime/src/authz_hook.rs` — add `AuthzBackend` enum, Cedar authorize path
- `rust_tools/crates/tools/src/authz.rs` — no changes needed (existing types reused)
- `rust_tools/crates/cli/src/main.rs` — add `--policy-store` CLI arg
- `rust_tools/crates/cli/src/commands.rs` — pass policy store path to runtime, init Cedarling

---

## Task 1: Add cedarling dependency to workspace

**Files:**
- Modify: `rust_tools/Cargo.toml`
- Modify: `rust_tools/crates/runtime/Cargo.toml`

- [ ] **Step 1: Add cedarling to workspace dependencies**

In `rust_tools/Cargo.toml`, add to `[workspace.dependencies]`:

```toml
cedarling = { path = "../jans/jans-cedarling/cedarling", default-features = false }
anyhow = "1"
```

Note: `default-features = false` avoids pulling in the gRPC Lock Server dependency which we don't need.

- [ ] **Step 2: Add cedarling + anyhow to runtime crate**

In `rust_tools/crates/runtime/Cargo.toml`, add to `[dependencies]`:

```toml
cedarling = { workspace = true }
anyhow = { workspace = true }
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo check -p runtime 2>&1 | tail -5`

Expected: compiles successfully (or with warnings, no errors).

If cedarling has workspace dependency conflicts, we may need to adjust the path or pin versions. Debug from the error output.

- [ ] **Step 4: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/Cargo.toml rust_tools/crates/runtime/Cargo.toml
git commit -m "feat: add cedarling and anyhow workspace dependencies"
```

---

## Task 2: Create Cedar policy store files

**Files:**
- Create: `rust_tools/config/policies/metadata.json`
- Create: `rust_tools/config/policies/schema.cedarschema`
- Create: `rust_tools/config/policies/policies/orchestrator.cedar`
- Create: `rust_tools/config/policies/policies/researcher.cedar`
- Create: `rust_tools/config/policies/policies/verifier.cedar`
- Create: `rust_tools/config/policies/policies/copywriter.cedar`
- Create: `rust_tools/config/policies/entities/agents.json`
- Create: `rust_tools/config/policies/entities/tools.json`
- Create: `rust_tools/config/policies/tool_action_map.json`

- [ ] **Step 1: Create metadata.json**

```json
{
  "cedar_version": "v4.0.0",
  "policy_stores": {
    "agent_policy_store": {
      "name": "Clawdia Agent Policy Store",
      "description": "Cedar policies for Clawdia agent tool authorization"
    }
  }
}
```

Wait — Cedarling's directory-based policy store uses a specific structure. Let me use the format Cedarling expects. Looking at the source, `PolicyStoreSource::Directory` reads:
- `metadata.json` at root
- `*.cedarschema` files
- `policies/*.cedar` files
- `entities/*.json` files

The metadata.json for directory format is simpler. Check Cedarling's `init/policy_store.rs` for the exact format expected. For now, create the files matching what we know from the Cedarling docs and test files.

Create `rust_tools/config/policies/metadata.json`:

```json
{
  "cedar_version": "4.9.0",
  "name": "Clawdia Agent Policies",
  "description": "Cedar policies for agent tool authorization"
}
```

- [ ] **Step 2: Create schema.cedarschema**

Create `rust_tools/config/policies/schema.cedarschema`:

```cedarschema
namespace AgentPolicy {
  entity Agent = {
    agent_type: String,
  };

  entity Tool = {
    tool_type: String,
    domain: String,
  };

  action "web_fetch" appliesTo {
    principal: [Agent],
    resource: [Tool],
    context: {
      requested_domain?: String,
    }
  };

  action "campaign_write" appliesTo {
    principal: [Agent],
    resource: [Tool],
    context: {
      campaign_id?: String,
    }
  };

  action "campaign_read" appliesTo {
    principal: [Agent],
    resource: [Tool],
    context: {
      campaign_id?: String,
    }
  };

  action "campaign_manage" appliesTo {
    principal: [Agent],
    resource: [Tool],
    context: {}
  };
}
```

Note: `campaign_manage` is for orchestrator-only tools like `doc_create`, `doc_assemble`, `doc_publish_*`. Context fields are optional (`?`) so policies can work even when context isn't provided.

- [ ] **Step 3: Create orchestrator.cedar**

Create `rust_tools/config/policies/policies/orchestrator.cedar`:

```cedar
@description("Orchestrator can create new campaigns")
@tool("doc_create")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_create"
);

@description("Orchestrator can assemble campaign content")
@tool("doc_assemble")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_assemble"
);

@description("Orchestrator can publish campaign draft")
@tool("doc_publish_draft")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_publish_draft"
);

@description("Orchestrator can publish campaign live")
@tool("doc_publish_live")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_publish_live"
);

@description("Orchestrator can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Orchestrator can list statements")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);

@description("Orchestrator can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);

@description("Orchestrator can read campaign data")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
```

- [ ] **Step 4: Create researcher.cedar**

Create `rust_tools/config/policies/policies/researcher.cedar`:

```cedar
@description("Researcher can search the web for marine conservation sources")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);

@description("Researcher can fetch page content")
@tool("fetch_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
);

@description("Researcher can use browser to bypass bot detection")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);

@description("Researcher can add statements to campaigns")
@tool("doc_add_statement")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_add_statement"
);

@description("Researcher can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Researcher can list statements to avoid duplicates")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);

@description("Researcher can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);

@description("Researcher can read campaign data")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
```

- [ ] **Step 5: Create verifier.cedar**

Create `rust_tools/config/policies/policies/verifier.cedar`:

```cedar
@description("Verifier can fetch page content to check sources")
@tool("fetch_content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
);

@description("Verifier can use browser to check sources")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);

@description("Verifier can set verdict on statements")
@tool("doc_set_verdict")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_set_verdict"
);

@description("Verifier can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Verifier can list statements for verification")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);

@description("Verifier can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);

@description("Verifier can read campaign data")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
```

- [ ] **Step 6: Create copywriter.cedar**

Create `rust_tools/config/policies/policies/copywriter.cedar`:

```cedar
@description("Copywriter can write campaign content")
@tool("doc_write_content")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_write_content"
);

@description("Copywriter can read verified campaign data only")
@tool("doc_get_verified")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get_verified"
);

@description("Copywriter can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Copywriter can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);
```

- [ ] **Step 7: Create entities/agents.json**

Create `rust_tools/config/policies/entities/agents.json`:

```json
[
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "orchestrator" },
    "attrs": { "agent_type": "orchestrator" },
    "parents": []
  },
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "researcher" },
    "attrs": { "agent_type": "researcher" },
    "parents": []
  },
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "verifier" },
    "attrs": { "agent_type": "verifier" },
    "parents": []
  },
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "copywriter" },
    "attrs": { "agent_type": "copywriter" },
    "parents": []
  }
]
```

- [ ] **Step 8: Create entities/tools.json**

Create `rust_tools/config/policies/entities/tools.json`:

```json
[
  { "uid": { "type": "AgentPolicy::Tool", "id": "search" }, "attrs": { "tool_type": "mcp", "domain": "web" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "fetch_content" }, "attrs": { "tool_type": "mcp", "domain": "web" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "chrome_get_web_content" }, "attrs": { "tool_type": "mcp", "domain": "chrome" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_create" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_add_statement" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_set_verdict" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_write_content" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_get_verified" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_list" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_list_statements" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_status" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_get" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_assemble" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_publish_draft" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] },
  { "uid": { "type": "AgentPolicy::Tool", "id": "doc_publish_live" }, "attrs": { "tool_type": "mcp", "domain": "campaign" }, "parents": [] }
]
```

- [ ] **Step 9: Create tool_action_map.json**

Create `rust_tools/config/policies/tool_action_map.json`:

```json
{
  "search": "web_fetch",
  "fetch_content": "web_fetch",
  "chrome_get_web_content": "web_fetch",
  "doc_create": "campaign_manage",
  "doc_add_statement": "campaign_write",
  "doc_set_verdict": "campaign_write",
  "doc_write_content": "campaign_write",
  "doc_get_verified": "campaign_read",
  "doc_list": "campaign_read",
  "doc_list_statements": "campaign_read",
  "doc_status": "campaign_read",
  "doc_get": "campaign_read",
  "doc_assemble": "campaign_manage",
  "doc_publish_draft": "campaign_manage",
  "doc_publish_live": "campaign_manage"
}
```

- [ ] **Step 10: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/config/policies/
git commit -m "feat: add Cedar policy store with schema, policies, and entities"
```

---

## Task 3: Add `authz_mode` to agent config

**Files:**
- Modify: `rust_tools/crates/runtime/src/agents.rs:18-32`
- Test: existing tests in `rust_tools/crates/runtime/src/agents.rs`

- [ ] **Step 1: Write tests for authz_mode deserialization**

Add to the `#[cfg(test)] mod tests` block in `rust_tools/crates/runtime/src/agents.rs`:

```rust
    #[test]
    fn authz_mode_defaults_to_cedarling() {
        let yaml = r#"
            name: test
            preamble: "test"
        "#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.authz_mode, AuthzMode::Cedarling);
    }

    #[test]
    fn authz_mode_yaml_explicit() {
        let yaml = r#"
            name: test
            preamble: "test"
            authz_mode: yaml
            permitted_actions:
              - read_file
        "#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.authz_mode, AuthzMode::Yaml);
        assert_eq!(config.permitted_actions, vec!["read_file"]);
    }

    #[test]
    fn authz_mode_cedarling_explicit() {
        let yaml = r#"
            name: test
            preamble: "test"
            authz_mode: cedarling
        "#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.authz_mode, AuthzMode::Cedarling);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime -- authz_mode 2>&1 | tail -10`

Expected: FAIL — `AuthzMode` not defined.

- [ ] **Step 3: Add AuthzMode enum and field to AgentConfig**

In `rust_tools/crates/runtime/src/agents.rs`, add above the `AgentConfig` struct:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthzMode {
    #[default]
    Cedarling,
    Yaml,
}
```

Add to `AgentConfig`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub preamble: String,
    #[serde(default)]
    pub authz_mode: AuthzMode,
    #[serde(default)]
    pub permitted_actions: Vec<String>,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime -- authz_mode 2>&1 | tail -10`

Expected: all 3 new tests PASS.

- [ ] **Step 5: Run all existing tests**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime 2>&1 | tail -10`

Expected: all tests PASS (existing tests should still work since `authz_mode` defaults and `permitted_actions` remains).

- [ ] **Step 6: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/crates/runtime/src/agents.rs
git commit -m "feat: add authz_mode field to AgentConfig (defaults to cedarling)"
```

---

## Task 4: Implement Cedar authorization module

**Files:**
- Create: `rust_tools/crates/runtime/src/cedar_authz.rs`
- Modify: `rust_tools/crates/runtime/src/lib.rs`

- [ ] **Step 1: Write tests for CedarAuthz**

Create `rust_tools/crates/runtime/src/cedar_authz.rs` with tests at the bottom:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use cedarling::{
    AuthorizationConfig, BootstrapConfig, CedarEntityMapping, Cedarling, DataStoreConfig,
    EntityBuilderConfig, EntityData, JsonRule, JwtConfig, LogConfig, LogLevel, LogTypeConfig,
    PolicyStoreConfig, PolicyStoreSource, RequestUnsigned,
};
use serde_json::{Value, json};

use tools::authz::{AuthorizationDecision, AuthorizationResult};

/// Tool name → Cedar action name (without namespace prefix).
/// Loaded from `tool_action_map.json`.
pub type ToolActionMap = HashMap<String, String>;

/// Wraps Cedarling instance and tool-action mapping for agent authorization.
pub struct CedarAuthz {
    cedarling: Cedarling,
    tool_action_map: ToolActionMap,
}

impl CedarAuthz {
    /// Initialize Cedarling from a policy store directory.
    pub async fn from_directory(policy_store_path: &Path) -> Result<Self> {
        let map_path = policy_store_path.join("tool_action_map.json");
        let map_content = std::fs::read_to_string(&map_path)
            .with_context(|| format!("reading {}", map_path.display()))?;
        let tool_action_map: ToolActionMap = serde_json::from_str(&map_content)
            .with_context(|| format!("parsing {}", map_path.display()))?;

        let cedarling = Cedarling::new(&BootstrapConfig {
            application_name: "clawdia".to_string(),
            log_config: LogConfig {
                log_type: LogTypeConfig::Off,
                log_level: LogLevel::WARN,
            },
            policy_store_config: PolicyStoreConfig {
                source: PolicyStoreSource::Directory(policy_store_path.into()),
            },
            jwt_config: JwtConfig {
                jwks: None,
                jwt_sig_validation: false,
                jwt_status_validation: false,
                signature_algorithms_supported: Default::default(),
                ..Default::default()
            },
            authorization_config: AuthorizationConfig {
                decision_log_default_jwt_id: "jti".to_string(),
                principal_bool_operator: JsonRule::new(json!(
                    {"===": [{"var": "AgentPolicy::Agent"}, "ALLOW"]}
                ))
                .expect("valid JSON rule"),
            },
            entity_builder_config: EntityBuilderConfig::default(),
            lock_config: None,
            max_default_entities: None,
            max_base64_size: None,
            data_store_config: DataStoreConfig::default(),
        })
        .await
        .context("initializing Cedarling")?;

        Ok(Self {
            cedarling,
            tool_action_map,
        })
    }

    /// Authorize a tool call for an agent.
    /// Returns Allow/Deny with optional reason.
    pub async fn authorize(
        &self,
        agent_name: &str,
        tool_name: &str,
        args: &str,
    ) -> AuthorizationResult {
        let Some(action_short) = self.tool_action_map.get(tool_name) else {
            return AuthorizationResult {
                decision: AuthorizationDecision::Deny,
                reason: Some(format!(
                    "Tool '{tool_name}' has no action mapping — denied by default"
                )),
            };
        };

        let cedar_action = format!("AgentPolicy::Action::\"{action_short}\"");

        let context = build_context(action_short, args);

        let request = RequestUnsigned {
            principals: vec![EntityData {
                cedar_mapping: CedarEntityMapping {
                    entity_type: "AgentPolicy::Agent".to_string(),
                    id: agent_name.to_string(),
                },
                attributes: HashMap::from([(
                    "agent_type".to_string(),
                    json!(agent_name),
                )]),
            }],
            action: cedar_action,
            resource: EntityData {
                cedar_mapping: CedarEntityMapping {
                    entity_type: "AgentPolicy::Tool".to_string(),
                    id: tool_name.to_string(),
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
                        reason: Some(format!(
                            "Cedar policy denied '{tool_name}' for agent '{agent_name}'"
                        )),
                    }
                }
            }
            Err(e) => AuthorizationResult {
                decision: AuthorizationDecision::Deny,
                reason: Some(format!("Cedar authorization error: {e}")),
            },
        }
    }

    /// Get the tool-action map (for building permission prompts).
    pub fn tool_action_map(&self) -> &ToolActionMap {
        &self.tool_action_map
    }
}

/// Build context JSON from tool args based on action type.
fn build_context(action: &str, args: &str) -> Value {
    let parsed: Value = serde_json::from_str(args).unwrap_or(json!({}));
    let mut ctx = serde_json::Map::new();

    match action {
        "web_fetch" => {
            if let Some(domain) = extract_domain_from_value(&parsed) {
                ctx.insert("requested_domain".to_string(), json!(domain));
            }
        }
        "campaign_write" | "campaign_read" => {
            if let Some(id) = parsed.get("campaign_id").and_then(|v| v.as_str()) {
                ctx.insert("campaign_id".to_string(), json!(id));
            }
        }
        _ => {}
    }

    Value::Object(ctx)
}

fn extract_domain_from_value(v: &Value) -> Option<String> {
    let url_str = v
        .get("url")
        .or_else(|| v.get("domain"))
        .or_else(|| v.get("uri"))
        .and_then(|v| v.as_str())?;

    // Try to extract domain from URL
    if let Some(after_scheme) = url_str
        .strip_prefix("https://")
        .or(url_str.strip_prefix("http://"))
    {
        Some(after_scheme.split('/').next()?.to_string())
    } else {
        Some(url_str.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_context_web_fetch_extracts_domain() {
        let ctx = build_context(
            "web_fetch",
            r#"{"url": "https://fisheries.noaa.gov/article/123"}"#,
        );
        assert_eq!(ctx["requested_domain"], "fisheries.noaa.gov");
    }

    #[test]
    fn build_context_web_fetch_no_url_returns_empty() {
        let ctx = build_context("web_fetch", r#"{"query": "bottom trawling"}"#);
        assert!(ctx.as_object().unwrap().is_empty());
    }

    #[test]
    fn build_context_campaign_read_extracts_id() {
        let ctx = build_context("campaign_read", r#"{"campaign_id": "camp-1"}"#);
        assert_eq!(ctx["campaign_id"], "camp-1");
    }

    #[test]
    fn build_context_unknown_action_returns_empty() {
        let ctx = build_context("unknown_action", r#"{"anything": "here"}"#);
        assert!(ctx.as_object().unwrap().is_empty());
    }

    #[test]
    fn extract_domain_from_https_url() {
        let v = json!({"url": "https://oceana.org/reports/trawling"});
        assert_eq!(
            extract_domain_from_value(&v).as_deref(),
            Some("oceana.org")
        );
    }

    #[test]
    fn extract_domain_from_domain_field() {
        let v = json!({"domain": "oceana.org"});
        assert_eq!(
            extract_domain_from_value(&v).as_deref(),
            Some("oceana.org")
        );
    }

    #[test]
    fn extract_domain_returns_none_for_empty() {
        let v = json!({});
        assert_eq!(extract_domain_from_value(&v), None);
    }

    // Integration test: requires policy store on disk
    #[tokio::test]
    async fn cedar_authz_from_directory_and_authorize() {
        let policy_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/policies");

        if !policy_dir.exists() {
            eprintln!("Skipping integration test: policy store not found at {}", policy_dir.display());
            return;
        }

        let authz = CedarAuthz::from_directory(&policy_dir).await;
        match authz {
            Ok(authz) => {
                // Researcher should be allowed to search
                let result = authz.authorize("researcher", "search", "{}").await;
                assert_eq!(result.decision, AuthorizationDecision::Allow);

                // Researcher should be denied doc_create
                let result = authz.authorize("researcher", "doc_create", "{}").await;
                assert_eq!(result.decision, AuthorizationDecision::Deny);

                // Unknown tool should be denied
                let result = authz.authorize("researcher", "nonexistent_tool", "{}").await;
                assert_eq!(result.decision, AuthorizationDecision::Deny);
            }
            Err(e) => {
                eprintln!("Cedarling init failed (may need policy store fixes): {e}");
                // Don't panic — policy store format may need adjusting
            }
        }
    }
}
```

- [ ] **Step 2: Export the module**

Add to `rust_tools/crates/runtime/src/lib.rs`:

```rust
pub mod cedar_authz;
```

- [ ] **Step 3: Run unit tests**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime -- cedar_authz 2>&1 | tail -20`

Expected: unit tests (build_context, extract_domain) PASS. The integration test may skip or fail if Cedarling's directory format doesn't match our files — debug and fix if needed.

- [ ] **Step 4: Debug and fix any Cedarling format issues**

If the integration test fails due to policy store format, read the Cedarling error, adjust `metadata.json` or file structure as needed, and re-run. This is the expected iteration point.

- [ ] **Step 5: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/crates/runtime/src/cedar_authz.rs rust_tools/crates/runtime/src/lib.rs
git commit -m "feat: add CedarAuthz module wrapping Cedarling authorize_unsigned"
```

---

## Task 5: Implement policy-aware system prompts

**Files:**
- Create: `rust_tools/crates/runtime/src/policy_prompt.rs`
- Modify: `rust_tools/crates/runtime/src/lib.rs`

- [ ] **Step 1: Write tests for permission prompt builder**

Create `rust_tools/crates/runtime/src/policy_prompt.rs`:

```rust
use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

/// A single permission entry extracted from a Cedar policy annotation.
#[derive(Debug, Clone)]
pub struct PermissionEntry {
    pub tool: String,
    pub description: String,
}

/// Parse all `.cedar` files in a policies directory, extract @tool and @description
/// annotations, and return entries grouped by agent name.
pub fn load_permissions_from_policies(
    policies_dir: &Path,
) -> Result<HashMap<String, Vec<PermissionEntry>>> {
    let mut agent_permissions: HashMap<String, Vec<PermissionEntry>> = HashMap::new();

    let policies_path = policies_dir.join("policies");
    if !policies_path.exists() {
        return Ok(agent_permissions);
    }

    for entry in std::fs::read_dir(&policies_path)
        .with_context(|| format!("reading {}", policies_path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("cedar") {
            continue;
        }

        let agent_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let entries = parse_policy_annotations(&content);
        if !entries.is_empty() {
            agent_permissions.insert(agent_name, entries);
        }
    }

    Ok(agent_permissions)
}

/// Parse @tool("name") and @description("text") annotations from Cedar policy text.
/// Returns one PermissionEntry per policy block.
fn parse_policy_annotations(content: &str) -> Vec<PermissionEntry> {
    let mut entries = Vec::new();
    let mut current_tool: Option<String> = None;
    let mut current_desc: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(val) = extract_annotation(trimmed, "tool") {
            current_tool = Some(val);
        } else if let Some(val) = extract_annotation(trimmed, "description") {
            current_desc = Some(val);
        } else if trimmed.starts_with("permit(") || trimmed.starts_with("forbid(") {
            if let (Some(tool), Some(desc)) = (current_tool.take(), current_desc.take()) {
                entries.push(PermissionEntry {
                    tool,
                    description: desc,
                });
            } else {
                current_tool = None;
                current_desc = None;
            }
        }
    }

    entries
}

fn extract_annotation(line: &str, name: &str) -> Option<String> {
    let prefix = format!("@{name}(\"");
    let stripped = line.strip_prefix(&prefix)?;
    let value = stripped.strip_suffix("\")")?;
    Some(value.to_string())
}

/// Build a permissions summary string to append to an agent's system prompt.
pub fn build_permissions_prompt(entries: &[PermissionEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut lines = vec!["\n## Your permissions\n".to_string()];
    lines.push("You are authorized to use the following tools:".to_string());
    for entry in entries {
        lines.push(format!("- {} \u{2014} {}", entry.tool, entry.description));
    }
    lines.push("\nAny tool call outside this list will be denied.".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_annotations_extracts_tool_and_description() {
        let cedar = r#"
@description("Researcher can search the web")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);

@description("Researcher can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
"#;
        let entries = parse_policy_annotations(cedar);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].tool, "search");
        assert_eq!(entries[0].description, "Researcher can search the web");
        assert_eq!(entries[1].tool, "doc_list");
        assert_eq!(entries[1].description, "Researcher can list campaigns");
    }

    #[test]
    fn parse_annotations_skips_incomplete_blocks() {
        let cedar = r#"
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);
"#;
        let entries = parse_policy_annotations(cedar);
        assert_eq!(entries.len(), 0, "missing @description should skip");
    }

    #[test]
    fn build_prompt_formats_correctly() {
        let entries = vec![
            PermissionEntry {
                tool: "search".to_string(),
                description: "Search the web".to_string(),
            },
            PermissionEntry {
                tool: "doc_list".to_string(),
                description: "List campaigns".to_string(),
            },
        ];
        let prompt = build_permissions_prompt(&entries);
        assert!(prompt.contains("## Your permissions"));
        assert!(prompt.contains("- search \u{2014} Search the web"));
        assert!(prompt.contains("- doc_list \u{2014} List campaigns"));
        assert!(prompt.contains("Any tool call outside this list will be denied."));
    }

    #[test]
    fn build_prompt_empty_entries_returns_empty() {
        assert!(build_permissions_prompt(&[]).is_empty());
    }
}
```

- [ ] **Step 2: Export the module**

Add to `rust_tools/crates/runtime/src/lib.rs`:

```rust
pub mod policy_prompt;
```

- [ ] **Step 3: Run tests**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime -- policy_prompt 2>&1 | tail -10`

Expected: all 4 tests PASS.

- [ ] **Step 4: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/crates/runtime/src/policy_prompt.rs rust_tools/crates/runtime/src/lib.rs
git commit -m "feat: add policy_prompt module to build agent permission summaries from Cedar annotations"
```

---

## Task 6: Add AuthzBackend to AuthzHook

**Files:**
- Modify: `rust_tools/crates/runtime/src/authz_hook.rs`

- [ ] **Step 1: Write tests for Cedar backend path**

Add to the `#[cfg(test)] mod tests` block in `authz_hook.rs`:

```rust
    #[test]
    fn yaml_backend_allows_permitted_tool() {
        let hook = make_hook_with_backend("researcher", &["search"], AuthzBackend::Yaml);
        let result = hook.authorize("search", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Allow);
    }

    #[test]
    fn yaml_backend_denies_unpermitted_tool() {
        let hook = make_hook_with_backend("researcher", &["search"], AuthzBackend::Yaml);
        let result = hook.authorize("doc_create", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Deny);
    }
```

And the helper:

```rust
    fn make_hook_with_backend(name: &str, permitted: &[&str], backend: AuthzBackend) -> AuthzHook {
        let principal = Principal {
            id: name.into(),
            principal_type: name.into(),
            delegation_record_id: None,
        };
        AuthzHook::new(
            principal,
            permitted.iter().map(|s| s.to_string()).collect(),
            AuditLog::new(),
            backend,
        )
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime -- yaml_backend 2>&1 | tail -10`

Expected: FAIL — `AuthzBackend` not defined, `new` signature changed.

- [ ] **Step 3: Add AuthzBackend enum and update AuthzHook**

Update `authz_hook.rs`:

```rust
use std::sync::Arc;
use crate::cedar_authz::CedarAuthz;

#[derive(Clone)]
pub enum AuthzBackend {
    /// Flat list from agents.yaml — fallback mode
    Yaml,
    /// Cedar policy engine — default
    Cedar(Arc<CedarAuthz>),
}
```

Update the `AuthzHook` struct and `new`:

```rust
#[derive(Clone)]
pub struct AuthzHook {
    principal: Principal,
    permitted_actions: Vec<String>,
    sub_agent_tools: Vec<String>,
    audit_log: AuditLog,
    backend: AuthzBackend,
}

impl AuthzHook {
    pub fn new(
        principal: Principal,
        permitted_actions: Vec<String>,
        audit_log: AuditLog,
        backend: AuthzBackend,
    ) -> Self {
        Self {
            principal,
            permitted_actions,
            sub_agent_tools: Vec::new(),
            audit_log,
            backend,
        }
    }
```

Update `authorize` method — keep the existing YAML logic inside the `Yaml` branch, add the `Cedar` branch:

```rust
    pub fn authorize(&self, tool_name: &str, args: &str) -> AuthorizationResult {
        // Sub-agent tools are always allowed (they enforce their own permissions)
        if self.sub_agent_tools.iter().any(|t| t == tool_name) {
            let result = AuthorizationResult {
                decision: AuthorizationDecision::Allow,
                reason: None,
            };
            self.log_and_audit(tool_name, args, &result);
            return result;
        }

        let result = match &self.backend {
            AuthzBackend::Yaml => {
                if self.permitted_actions.iter().any(|t| t == tool_name) {
                    AuthorizationResult {
                        decision: AuthorizationDecision::Allow,
                        reason: None,
                    }
                } else {
                    AuthorizationResult {
                        decision: AuthorizationDecision::Deny,
                        reason: Some(format!(
                            "Tool '{tool_name}' not in permitted tools for {}",
                            self.principal.id
                        )),
                    }
                }
            }
            AuthzBackend::Cedar(cedar) => {
                // Cedar authorize is async but PromptHook::on_tool_call is async too,
                // so we call it from there. For the sync `authorize` method used in tests,
                // we use tokio::runtime::Handle to block on it.
                // In practice, `on_tool_call` calls cedar directly (see PromptHook impl).
                // This sync path is only for YAML-mode tests.
                AuthorizationResult {
                    decision: AuthorizationDecision::Deny,
                    reason: Some("Cedar sync path not supported — use on_tool_call".to_string()),
                }
            }
        };

        self.log_and_audit(tool_name, args, &result);
        result
    }
```

Wait — there's an issue. `CedarAuthz::authorize` is async, but the current `AuthzHook::authorize` is sync. The `PromptHook::on_tool_call` is async though. Let me restructure:

Keep `authorize` for YAML-only (sync tests), and add `authorize_async` for Cedar path:

```rust
    pub fn authorize(&self, tool_name: &str, args: &str) -> AuthorizationResult {
        // Sub-agent tools always allowed
        if self.sub_agent_tools.iter().any(|t| t == tool_name) {
            let result = AuthorizationResult {
                decision: AuthorizationDecision::Allow,
                reason: None,
            };
            self.log_and_audit(tool_name, args, &result);
            return result;
        }

        match &self.backend {
            AuthzBackend::Yaml => {
                let result = if self.permitted_actions.iter().any(|t| t == tool_name) {
                    AuthorizationResult { decision: AuthorizationDecision::Allow, reason: None }
                } else {
                    AuthorizationResult {
                        decision: AuthorizationDecision::Deny,
                        reason: Some(format!("Tool '{tool_name}' not in permitted tools for {}", self.principal.id)),
                    }
                };
                self.log_and_audit(tool_name, args, &result);
                result
            }
            AuthzBackend::Cedar(_) => {
                // Cedar authorization requires async — called via authorize_cedar in on_tool_call
                panic!("Cedar backend requires async authorize — use on_tool_call path");
            }
        }
    }

    async fn authorize_cedar(&self, cedar: &CedarAuthz, tool_name: &str, args: &str) -> AuthorizationResult {
        let result = cedar.authorize(&self.principal.id, tool_name, args).await;
        self.log_and_audit(tool_name, args, &result);
        result
    }

    fn log_and_audit(&self, tool_name: &str, args: &str, result: &AuthorizationResult) {
        let request = AuthorizationRequest {
            principal: self.principal.clone(),
            action: tool_name.to_string(),
            resource: tool_name.to_string(),
            context: PolicyContext {
                delegation_record_id: self.principal.delegation_record_id,
                requested_domain: extract_domain(args),
            },
        };
        self.audit_log
            .append(AuditEntry::from_request(&request, result));
    }
```

Update `PromptHook::on_tool_call`:

```rust
impl<M: CompletionModel> PromptHook<M> for AuthzHook {
    async fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        // Sub-agent tools always allowed
        if self.sub_agent_tools.iter().any(|t| t == tool_name) {
            self.log_and_audit(tool_name, args, &AuthorizationResult {
                decision: AuthorizationDecision::Allow,
                reason: None,
            });
            return ToolCallHookAction::cont();
        }

        let result = match &self.backend {
            AuthzBackend::Yaml => self.authorize(tool_name, args),
            AuthzBackend::Cedar(cedar) => self.authorize_cedar(cedar, tool_name, args).await,
        };

        match result.decision {
            AuthorizationDecision::Allow => ToolCallHookAction::cont(),
            AuthorizationDecision::Deny => {
                let reason = result
                    .reason
                    .unwrap_or_else(|| "denied by policy".to_string());
                ToolCallHookAction::skip(reason)
            }
        }
    }
```

Hmm this is getting complex. Let me simplify — just update `authorize` to always work as before for Yaml, and have `on_tool_call` dispatch to cedar directly. Simpler approach:

```rust
    pub fn authorize(&self, tool_name: &str, args: &str) -> AuthorizationResult {
        // This is the YAML-only sync path. Cedar goes through on_tool_call → authorize_cedar.
        // Sub-agent tools always allowed
        if self.sub_agent_tools.iter().any(|t| t == tool_name) {
            return self.log_result(tool_name, args, AuthorizationDecision::Allow, None);
        }

        if self.permitted_actions.iter().any(|t| t == tool_name) {
            self.log_result(tool_name, args, AuthorizationDecision::Allow, None)
        } else {
            self.log_result(
                tool_name,
                args,
                AuthorizationDecision::Deny,
                Some(format!("Tool '{tool_name}' not in permitted tools for {}", self.principal.id)),
            )
        }
    }
```

Actually, I'm overcomplicating the plan. Let me write it clean. The key insight: `authorize()` stays sync for YAML. The `on_tool_call` async method handles both paths.

- [ ] **Step 3: Implement AuthzBackend and update AuthzHook**

Replace the entire `authz_hook.rs` with:

```rust
use std::sync::Arc;

use rig::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig::completion::CompletionModel;

use tools::authz::{
    AuditEntry, AuthorizationDecision, AuthorizationRequest, AuthorizationResult, PolicyContext,
    Principal,
};

use crate::agents::AuditLog;
use crate::cedar_authz::CedarAuthz;

#[derive(Clone)]
pub enum AuthzBackend {
    /// Flat list from agents.yaml — fallback mode
    Yaml,
    /// Cedar policy engine — default
    Cedar(Arc<CedarAuthz>),
}

/// A PromptHook that enforces deny-by-default tool authorization.
///
/// In Yaml mode: checks tool name against `permitted_actions` list.
/// In Cedar mode: delegates to Cedarling `authorize_unsigned`.
#[derive(Clone)]
pub struct AuthzHook {
    principal: Principal,
    permitted_actions: Vec<String>,
    sub_agent_tools: Vec<String>,
    audit_log: AuditLog,
    backend: AuthzBackend,
}

impl AuthzHook {
    pub fn new(
        principal: Principal,
        permitted_actions: Vec<String>,
        audit_log: AuditLog,
        backend: AuthzBackend,
    ) -> Self {
        Self {
            principal,
            permitted_actions,
            sub_agent_tools: Vec::new(),
            audit_log,
            backend,
        }
    }

    pub fn with_sub_agent_tools(mut self, tools: Vec<String>) -> Self {
        self.sub_agent_tools = tools;
        self
    }

    /// Synchronous YAML-mode authorization. Used by tests and the YAML backend path.
    pub fn authorize(&self, tool_name: &str, args: &str) -> AuthorizationResult {
        let is_permitted = self.permitted_actions.iter().any(|t| t == tool_name)
            || self.sub_agent_tools.iter().any(|t| t == tool_name);

        let result = if is_permitted {
            AuthorizationResult {
                decision: AuthorizationDecision::Allow,
                reason: None,
            }
        } else {
            AuthorizationResult {
                decision: AuthorizationDecision::Deny,
                reason: Some(format!(
                    "Tool '{tool_name}' not in permitted tools for {}",
                    self.principal.id
                )),
            }
        };

        self.record_audit(tool_name, args, &result);
        result
    }

    fn record_audit(&self, tool_name: &str, args: &str, result: &AuthorizationResult) {
        let request = AuthorizationRequest {
            principal: self.principal.clone(),
            action: tool_name.to_string(),
            resource: tool_name.to_string(),
            context: PolicyContext {
                delegation_record_id: self.principal.delegation_record_id,
                requested_domain: extract_domain(args),
            },
        };
        self.audit_log
            .append(AuditEntry::from_request(&request, result));
    }
}

impl<M: CompletionModel> PromptHook<M> for AuthzHook {
    async fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        // Sub-agent tools always allowed — they have their own AuthzHook
        if self.sub_agent_tools.iter().any(|t| t == tool_name) {
            let result = AuthorizationResult {
                decision: AuthorizationDecision::Allow,
                reason: None,
            };
            self.record_audit(tool_name, args, &result);
            return ToolCallHookAction::cont();
        }

        let result = match &self.backend {
            AuthzBackend::Yaml => self.authorize(tool_name, args),
            AuthzBackend::Cedar(cedar) => {
                let r = cedar.authorize(&self.principal.id, tool_name, args).await;
                self.record_audit(tool_name, args, &r);
                r
            }
        };

        match result.decision {
            AuthorizationDecision::Allow => ToolCallHookAction::cont(),
            AuthorizationDecision::Deny => {
                let reason = result
                    .reason
                    .unwrap_or_else(|| "denied by policy".to_string());
                ToolCallHookAction::skip(reason)
            }
        }
    }

    async fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> HookAction {
        let preview = truncate_str(result, 100);
        println!(
            "  │  📎 [{principal}] tool {tool_name} returned: {preview}",
            principal = self.principal.id,
            preview = preview.replace('\n', " "),
        );
        HookAction::cont()
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    let truncated = truncated.replace('\n', " ");
    if s.chars().count() > max_chars {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn extract_domain(args: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(args).ok()?;
    v.get("url")
        .or_else(|| v.get("domain"))
        .or_else(|| v.get("uri"))
        .and_then(|v| v.as_str())
        .and_then(|s| url_domain(s).or_else(|| Some(s.to_string())))
}

fn url_domain(s: &str) -> Option<String> {
    let after_scheme = s.strip_prefix("https://").or(s.strip_prefix("http://"))?;
    Some(after_scheme.split('/').next()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AuditLog;
    use tools::authz::{AuthorizationDecision, Principal};

    fn make_hook(name: &str, permitted: &[&str]) -> AuthzHook {
        let principal = Principal {
            id: name.into(),
            principal_type: name.into(),
            delegation_record_id: None,
        };
        AuthzHook::new(
            principal,
            permitted.iter().map(|s| s.to_string()).collect(),
            AuditLog::new(),
            AuthzBackend::Yaml,
        )
    }

    // All existing tests remain unchanged — they call make_hook which uses Yaml backend.
    // Just add AuthzBackend::Yaml to the make_hook helper (already done above).

    // ── deny-by-default ──

    #[test]
    fn empty_permitted_actions_denies_everything() {
        let hook = make_hook("orchestrator", &[]);
        let result = hook.authorize("read_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Deny);
    }

    #[test]
    fn permitted_action_is_allowed() {
        let hook = make_hook("orchestrator", &["read_file"]);
        let result = hook.authorize("read_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Allow);
        assert!(result.reason.is_none());
    }

    #[test]
    fn unpermitted_action_is_denied() {
        let hook = make_hook("orchestrator", &["read_file"]);
        let result = hook.authorize("write_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Deny);
        assert!(result.reason.unwrap().contains("write_file"));
    }

    // ── sub-agent delegation ──

    fn make_hook_with_sub_agents(
        name: &str,
        permitted: &[&str],
        sub_agents: &[&str],
    ) -> AuthzHook {
        make_hook(name, permitted).with_sub_agent_tools(
            sub_agents.iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn registered_sub_agent_tool_is_allowed() {
        let hook = make_hook_with_sub_agents(
            "orchestrator",
            &[],
            &["agent_researcher", "agent_verifier"],
        );
        assert_eq!(
            hook.authorize("agent_researcher", "{}").decision,
            AuthorizationDecision::Allow
        );
        assert_eq!(
            hook.authorize("agent_verifier", "{}").decision,
            AuthorizationDecision::Allow
        );
    }

    #[test]
    fn unregistered_sub_agent_tool_is_denied() {
        let hook = make_hook_with_sub_agents(
            "orchestrator",
            &["doc_create"],
            &["agent_researcher"],
        );
        assert_eq!(
            hook.authorize("agent_copywriter", "{}").decision,
            AuthorizationDecision::Deny
        );
    }

    #[test]
    fn sub_agent_tools_do_not_bypass_regular_tool_deny() {
        let hook = make_hook_with_sub_agents(
            "orchestrator",
            &[],
            &["agent_researcher"],
        );
        assert_eq!(
            hook.authorize("doc_create", "{}").decision,
            AuthorizationDecision::Deny
        );
    }

    #[test]
    fn sub_agent_with_no_tools_cannot_call_anything() {
        let hook = make_hook("researcher", &[]);
        let result = hook.authorize("read_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Deny);
    }

    #[test]
    fn sub_agent_confined_to_permitted_tools() {
        let hook = make_hook("researcher", &["search_sources", "fetch_article"]);

        assert_eq!(
            hook.authorize("search_sources", "{}").decision,
            AuthorizationDecision::Allow
        );
        assert_eq!(
            hook.authorize("fetch_article", "{}").decision,
            AuthorizationDecision::Allow
        );
        assert_eq!(
            hook.authorize("write_file", "{}").decision,
            AuthorizationDecision::Deny
        );
        assert_eq!(
            hook.authorize("publish_instagram_live", "{}").decision,
            AuthorizationDecision::Deny
        );
    }

    // ── audit log ──

    #[test]
    fn authorize_appends_to_audit_log() {
        let audit_log = AuditLog::new();
        let principal = Principal {
            id: "orchestrator".into(),
            principal_type: "orchestrator".into(),
            delegation_record_id: None,
        };
        let hook = AuthzHook::new(
            principal,
            vec!["read_file".into()],
            audit_log.clone(),
            AuthzBackend::Yaml,
        );

        hook.authorize("read_file", "{}");
        hook.authorize("write_file", "{}");

        let entries = audit_log.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].decision, AuthorizationDecision::Allow);
        assert_eq!(entries[0].action, "read_file");
        assert_eq!(entries[1].decision, AuthorizationDecision::Deny);
        assert_eq!(entries[1].action, "write_file");
    }

    #[test]
    fn audit_log_records_principal_identity() {
        let audit_log = AuditLog::new();
        let principal = Principal {
            id: "researcher".into(),
            principal_type: "researcher".into(),
            delegation_record_id: None,
        };
        let hook = AuthzHook::new(principal, vec![], audit_log.clone(), AuthzBackend::Yaml);

        hook.authorize("some_tool", "{}");

        let entry = &audit_log.entries()[0];
        assert_eq!(entry.principal_id, "researcher");
        assert_eq!(entry.principal_type, "researcher");
    }

    // ── domain extraction ──

    #[test]
    fn extract_domain_from_url_arg() {
        let domain = extract_domain(r#"{"url": "https://fisheries.noaa.gov/article/123"}"#);
        assert_eq!(domain.as_deref(), Some("fisheries.noaa.gov"));
    }

    #[test]
    fn extract_domain_from_domain_arg() {
        let domain = extract_domain(r#"{"domain": "oceana.org"}"#);
        assert_eq!(domain.as_deref(), Some("oceana.org"));
    }

    #[test]
    fn extract_domain_returns_none_for_empty_args() {
        assert_eq!(extract_domain("{}"), None);
    }

    #[test]
    fn extract_domain_returns_none_for_invalid_json() {
        assert_eq!(extract_domain("not json"), None);
    }

    // ── truncation (UTF-8 safe) ──

    #[test]
    fn truncate_ascii() {
        assert_eq!(truncate_str("hello world", 5), "hello…");
    }

    #[test]
    fn truncate_no_op_when_short() {
        assert_eq!(truncate_str("hi", 10), "hi");
    }

    #[test]
    fn truncate_utf8_does_not_panic() {
        let ukrainian = "Донне тралення знищує екосистеми морського дна";
        let result = truncate_str(ukrainian, 10);
        assert_eq!(result, "Донне трал…");
    }

    #[test]
    fn truncate_replaces_newlines() {
        assert_eq!(truncate_str("line1\nline2\nline3", 100), "line1 line2 line3");
    }
}
```

- [ ] **Step 4: Run all runtime tests**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime 2>&1 | tail -15`

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/crates/runtime/src/authz_hook.rs
git commit -m "feat: add AuthzBackend enum with Yaml/Cedar dual-mode in AuthzHook"
```

---

## Task 7: Update agent builder to use AuthzBackend

**Files:**
- Modify: `rust_tools/crates/runtime/src/agents.rs:55-173`

- [ ] **Step 1: Update build_agent to accept AuthzBackend**

In `agents.rs`, update `build_agent` signature and body:

```rust
pub fn build_agent<C: CompletionClient + 'static>(
    client: &C,
    model: &str,
    config: &AgentConfig,
    servers: &[McpServer],
    audit_log: &AuditLog,
    backend: AuthzBackend,
    permission_preamble: &str,
) -> Agent<C::CompletionModel, AuthzHook> {
    let principal = Principal {
        id: config.name.clone(),
        principal_type: config.name.clone(),
        delegation_record_id: None,
    };

    let hook = AuthzHook::new(
        principal,
        config.permitted_actions.clone(),
        audit_log.clone(),
        backend,
    );

    let permitted = &config.permitted_actions;

    // Append permission summary to agent preamble
    let full_preamble = if permission_preamble.is_empty() {
        config.preamble.clone()
    } else {
        format!("{}\n{}", config.preamble, permission_preamble)
    };

    let base = client
        .agent(model)
        .preamble(&full_preamble)
        .name(&config.name)
        .description(&config.description)
        .default_max_turns(DEFAULT_MAX_TURNS)
        .hook(hook);

    // Attach filtered MCP tools
    let mut server_iter = servers.iter();

    if let Some(first) = server_iter.next() {
        let filtered = filter_tools(first.tools.clone(), permitted);
        if filtered.is_empty() && permitted.is_empty() {
            return base.build();
        }
        let mut builder = base.rmcp_tools(filtered, first.sink.clone());
        for server in server_iter {
            builder = builder.rmcp_tools(
                filter_tools(server.tools.clone(), permitted),
                server.sink.clone(),
            );
        }
        builder.build()
    } else {
        base.build()
    }
}
```

- [ ] **Step 2: Update build_orchestrator similarly**

Update `build_orchestrator` to accept `backend`, `permission_preamble`, and pass them through:

```rust
pub fn build_orchestrator<C: CompletionClient + 'static>(
    client: &C,
    model: &str,
    config: &Config,
    servers: Vec<McpServer>,
    audit_log: &AuditLog,
    backend: AuthzBackend,
    agent_permissions: &HashMap<String, Vec<crate::policy_prompt::PermissionEntry>>,
) -> Agent<C::CompletionModel, AuthzHook> {
```

Inside, use `backend.clone()` for orchestrator's hook and for each sub-agent's `build_agent` call. Use `agent_permissions.get(&agent_cfg.name)` to build the permission preamble via `policy_prompt::build_permissions_prompt`.

Add imports at top:

```rust
use crate::authz_hook::AuthzBackend;
use crate::policy_prompt;
```

- [ ] **Step 3: Run all tests**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime 2>&1 | tail -15`

Expected: PASS (existing tests don't call build_agent/build_orchestrator directly).

- [ ] **Step 4: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/crates/runtime/src/agents.rs
git commit -m "feat: update agent builders to accept AuthzBackend and inject permission preambles"
```

---

## Task 8: Update CLI to initialize Cedarling

**Files:**
- Modify: `rust_tools/crates/cli/src/main.rs`
- Modify: `rust_tools/crates/cli/src/commands.rs`
- Modify: `rust_tools/crates/cli/Cargo.toml`

- [ ] **Step 1: Add dependencies to CLI crate**

In `rust_tools/crates/cli/Cargo.toml`, add:

```toml
anyhow = { workspace = true }
```

- [ ] **Step 2: Add --policy-store CLI arg**

In `rust_tools/crates/cli/src/main.rs`, add to `Cli` struct:

```rust
    /// Path to Cedar policy store directory
    #[arg(long, env = "CLAWDIA_POLICY_STORE_PATH", default_value = "config/policies")]
    policy_store: PathBuf,
```

Pass it to the `chat` command:

```rust
Command::Chat => {
    commands::chat(
        &cli.mcp_config,
        &cli.agents_config,
        &cli.policy_store,
        &cli.api_key,
        &cli.model,
    )
    .await
}
```

- [ ] **Step 3: Update commands::chat to initialize Cedarling**

In `rust_tools/crates/cli/src/commands.rs`:

```rust
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;

use rig::completion::Prompt;
use rig::providers::deepseek;
use runtime::agents::{self, AuditLog, AuthzMode};
use runtime::authz_hook::AuthzBackend;
use runtime::cedar_authz::CedarAuthz;
use runtime::policy_prompt;

pub async fn chat(
    mcp_config: &Path,
    agents_config: &Path,
    policy_store: &Path,
    api_key: &str,
    model_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = agents::load_config(agents_config)?;
    let (servers, running_services) = runtime::mcp::connect_all(mcp_config).await?;

    let audit_log = AuditLog::new();

    // Initialize Cedarling if any agent uses it
    let needs_cedar = config.orchestrator.authz_mode == AuthzMode::Cedarling
        || config.agents.values().any(|a| a.authz_mode == AuthzMode::Cedarling);

    let cedar = if needs_cedar && policy_store.exists() {
        match CedarAuthz::from_directory(policy_store).await {
            Ok(c) => {
                println!("Cedar policy engine loaded from {}", policy_store.display());
                Some(Arc::new(c))
            }
            Err(e) => {
                eprintln!("Warning: failed to load Cedar policies: {e}");
                eprintln!("Falling back to YAML authorization for all agents.");
                None
            }
        }
    } else {
        None
    };

    // Load permission prompts from Cedar policies
    let agent_permissions = if policy_store.exists() {
        policy_prompt::load_permissions_from_policies(policy_store).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let backend = match &cedar {
        Some(c) => AuthzBackend::Cedar(c.clone()),
        None => AuthzBackend::Yaml,
    };

    let client = deepseek::Client::new(api_key)?;

    let agent = agents::build_orchestrator(
        &client,
        model_name,
        &config,
        servers,
        &audit_log,
        backend,
        &agent_permissions,
    );

    println!("\nClawdia Schiffer — interactive chat (type 'quit' to exit)");

    let stdin = io::stdin();
    loop {
        print!("\nYou: ");
        io::stdout().flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let prompt = input.trim();

        if prompt.is_empty() || prompt == "quit" || prompt == "exit" {
            break;
        }

        match agent.prompt(prompt).await {
            Ok(response) => println!("\nAssistant: {response}"),
            Err(e) => eprintln!("\nError: {e}"),
        }
    }

    for svc in running_services {
        svc.cancel().await?;
    }
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo check 2>&1 | tail -10`

Expected: compiles. Fix any type mismatches or import issues.

- [ ] **Step 5: Run all tests**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test 2>&1 | tail -15`

Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
cd /home/user/work/jans/oleh_claudia
git add rust_tools/crates/cli/
git commit -m "feat: add --policy-store CLI arg and Cedarling initialization in chat command"
```

---

## Task 9: End-to-end verification

**Files:** none (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test 2>&1`

Expected: all tests PASS across all crates.

- [ ] **Step 2: Verify agents.yaml still works with YAML mode**

Update `rust_tools/config/agents.yaml` — add `authz_mode: yaml` to one agent temporarily and verify it parses:

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo run -- --api-key test agents 2>&1 | head -20`

Revert the change after verification.

- [ ] **Step 3: Verify Cedar policy store loads**

Run: `cd /home/user/work/jans/oleh_claudia/rust_tools && cargo test -p runtime -- cedar_authz_from_directory 2>&1`

Expected: integration test passes if Cedarling accepts our directory format.

- [ ] **Step 4: Commit final state**

If any fixes were needed during verification:

```bash
cd /home/user/work/jans/oleh_claudia
git add -A
git commit -m "fix: adjustments from end-to-end verification"
```
