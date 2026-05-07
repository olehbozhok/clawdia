use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::completion::{CompletionModel, Prompt, PromptError, ToolDefinition};
use rig::tool::Tool;

use crate::approvals::tools::{
    ApprovalDescribeTool, ApprovalExecuteTool, ApprovalListMineTool, ApprovalRequestTool,
    ApprovalStatusTool,
};
use crate::authz_hook::{AuthzBackend, AuthzHook};
use crate::mcp::McpServer;
use crate::notifications::tool::NotifyHumanTool;

use crate::runtime::Runtime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools::authz::{AuditEntry, Principal};

// ── Config types ──

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub orchestrator: AgentConfig,
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthzMode {
    #[default]
    Cedarling,
    Yaml,
}

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

pub fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

#[cfg(test)]
mod yaml_tests {
    use super::load_config;
    use crate::sub_agent::BUILTIN_AGENT_TOOLS;
    use std::path::PathBuf;

    fn shipped_config() -> super::Config {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/agents.yaml");
        load_config(&path).expect("agents.yaml must load")
    }

    #[test]
    fn approval_tools_are_builtin_not_yaml_permissions() {
        // approval_* are runtime-internal lifecycle tools, not Cedar-gated
        // privileges. They live in BUILTIN_AGENT_TOOLS and must NEVER appear
        // in any agent's permitted_actions — duplication would imply they
        // could be revoked, which would break gated-action recovery.
        let cfg = shipped_config();
        let mut all_lists: Vec<&Vec<String>> = vec![&cfg.orchestrator.permitted_actions];
        all_lists.extend(cfg.agents.values().map(|a| &a.permitted_actions));
        for tool in [
            "approval_request",
            "approval_status",
            "approval_describe",
            "approval_execute",
            "approval_list_mine",
        ] {
            assert!(
                BUILTIN_AGENT_TOOLS.contains(&tool),
                "{tool} must be in BUILTIN_AGENT_TOOLS"
            );
            for list in &all_lists {
                assert!(
                    !list.iter().any(|p| p == tool),
                    "{tool} is built-in; must not appear in permitted_actions: {list:?}"
                );
            }
        }
    }
}

pub const DEFAULT_MAX_TURNS: usize = 40;

// ── Tool filtering ──

/// Filter MCP tools to only include those in the permitted actions list.
pub fn filter_tools(tools: Vec<rmcp::model::Tool>, permitted: &[String]) -> Vec<rmcp::model::Tool> {
    tools
        .into_iter()
        .filter(|t| permitted.iter().any(|p| p.as_str() == t.name.as_ref()))
        .collect()
}

// ── Unified agent builder ──

/// Build an agent with filtered MCP tools and an AuthzHook.
/// This is the single build path for both orchestrator and sub-agents.
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

// ── AgentRuntime + runtime-backed builders (Plan 06) ──

/// Lightweight per-agent handle. Each running agent gets its own `AgentRuntime`
/// with its `SessionId` captured in every attached builtin tool.
pub struct AgentRuntime<M: CompletionModel> {
    pub agent: Agent<M, AuthzHook>,
    pub session_id: crate::sessions::SessionId,
    pub audit: Arc<AuditLog>,
}

/// Build the orchestrator agent with runtime-builtin tools.
/// Attaches approval_* + notify_human tools. `agent_spawn` deferred to Plan 07.
pub fn build_orchestrator<C: CompletionClient + 'static>(
    runtime: &Runtime,
    sid: crate::sessions::SessionId,
    client: &C,
    model: &str,
    yaml_cfg: &AgentConfig,
    servers: &[McpServer],
) -> anyhow::Result<AgentRuntime<C::CompletionModel>> {
    let principal = Principal {
        id: yaml_cfg.name.clone(),
        principal_type: yaml_cfg.name.clone(),
        delegation_record_id: None,
    };

    let hook = AuthzHook::new(
        principal,
        yaml_cfg.permitted_actions.clone(),
        (*runtime.audit_log).clone(),
        runtime.authz_backend.clone(),
    );

    let full_preamble = yaml_cfg.preamble.clone();

    let base = client
        .agent(model)
        .preamble(&full_preamble)
        .name(&yaml_cfg.name)
        .description(&yaml_cfg.description)
        .default_max_turns(DEFAULT_MAX_TURNS)
        .hook(hook);

    // Attach filtered MCP tools
    let mut groups = servers.iter();
    let first = groups.next().ok_or_else(|| {
        anyhow::anyhow!("At least one MCP server must be configured")
    })?;
    let permitted = &yaml_cfg.permitted_actions;
    let mut agent_builder = base.rmcp_tools(
        filter_tools(first.tools.clone(), permitted),
        first.sink.clone(),
    );
    for server in groups {
        agent_builder = agent_builder.rmcp_tools(
            filter_tools(server.tools.clone(), permitted),
            server.sink.clone(),
        );
    }

    // Attach builtin runtime tools
    let agent = agent_builder
        .tool(ApprovalRequestTool {
            gateway: runtime.gateway.clone(),
            caller_session_id: sid.clone(),
        })
        .tool(ApprovalStatusTool {
            gateway: runtime.gateway.clone(),
        })
        .tool(ApprovalDescribeTool {
            gateway: runtime.gateway.clone(),
        })
        .tool(ApprovalExecuteTool {
            gateway: runtime.gateway.clone(),
            registry: runtime.action_registry.clone(),
            caller_session_id: sid.clone(),
        })
        .tool(ApprovalListMineTool {
            gateway: runtime.gateway.clone(),
            caller_session_id: sid.clone(),
        })
        .tool(NotifyHumanTool {
            store: runtime.notification_store.clone(),
            idgen: runtime.notification_idgen.clone(),
            caller_session_id: sid.clone(),
        })
        // AgentSpawnTool — DEFERRED to Plan 07 (needs production ChildRunner)
        .build();

    Ok(AgentRuntime {
        agent,
        session_id: sid,
        audit: runtime.audit_log.clone(),
    })
}

/// Build a child agent with runtime-builtin tools.
/// Same as `build_orchestrator` but WITHOUT `NotifyHumanTool` (orchestrator-only
/// per Cedar policy). `agent_spawn` deferred to Plan 07.
pub fn build_child_agent<C: CompletionClient + 'static>(
    runtime: &Runtime,
    sid: crate::sessions::SessionId,
    client: &C,
    model: &str,
    yaml_cfg: &AgentConfig,
    servers: &[McpServer],
) -> anyhow::Result<AgentRuntime<C::CompletionModel>> {
    let principal = Principal {
        id: yaml_cfg.name.clone(),
        principal_type: yaml_cfg.name.clone(),
        delegation_record_id: None,
    };

    let hook = AuthzHook::new(
        principal,
        yaml_cfg.permitted_actions.clone(),
        (*runtime.audit_log).clone(),
        runtime.authz_backend.clone(),
    );

    let full_preamble = yaml_cfg.preamble.clone();

    let base = client
        .agent(model)
        .preamble(&full_preamble)
        .name(&yaml_cfg.name)
        .description(&yaml_cfg.description)
        .default_max_turns(DEFAULT_MAX_TURNS)
        .hook(hook);

    let permitted = &yaml_cfg.permitted_actions;
    let mut groups = servers.iter();
    let first = groups.next().ok_or_else(|| {
        anyhow::anyhow!("At least one MCP server must be configured")
    })?;
    let mut agent_builder = base.rmcp_tools(
        filter_tools(first.tools.clone(), permitted),
        first.sink.clone(),
    );
    for server in groups {
        agent_builder = agent_builder.rmcp_tools(
            filter_tools(server.tools.clone(), permitted),
            server.sink.clone(),
        );
    }

    let agent = agent_builder
        .tool(ApprovalRequestTool {
            gateway: runtime.gateway.clone(),
            caller_session_id: sid.clone(),
        })
        .tool(ApprovalStatusTool {
            gateway: runtime.gateway.clone(),
        })
        .tool(ApprovalDescribeTool {
            gateway: runtime.gateway.clone(),
        })
        .tool(ApprovalExecuteTool {
            gateway: runtime.gateway.clone(),
            registry: runtime.action_registry.clone(),
            caller_session_id: sid.clone(),
        })
        .tool(ApprovalListMineTool {
            gateway: runtime.gateway.clone(),
            caller_session_id: sid.clone(),
        })
        // NotifyHumanTool deliberately omitted — orchestrator-only per Cedar policy.
        // AgentSpawnTool — DEFERRED to Plan 07 (needs production ChildRunner)
        .build();

    Ok(AgentRuntime {
        agent,
        session_id: sid,
        audit: runtime.audit_log.clone(),
    })
}

// ── Audit log (shared across agents) ──

#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Arc<Mutex<Vec<AuditEntry>>>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&self, entry: AuditEntry) {
        tracing::info!(
            principal = %entry.principal_id,
            action = %entry.action,
            resource = %entry.resource,
            decision = ?entry.decision,
            denial_reason = ?entry.denial_reason,
            "authz decision",
        );
        self.entries.lock().expect("lock poisoned").push(entry);
    }

    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().expect("lock poisoned").clone()
    }
}

// ── Verbose wrapper ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentToolArgs {
    prompt: String,
}

/// Wraps an `Agent` to print status when the sub-agent is called.
/// Authorization is handled by `AuthzHook` attached to each agent.
pub struct VerboseAgent<M: CompletionModel> {
    inner: Agent<M, AuthzHook>,
    label: String,
}

impl<M: CompletionModel + 'static> Tool for VerboseAgent<M> {
    const NAME: &'static str = "agent_tool";

    type Error = PromptError;
    type Args = AgentToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let description = format!(
            "Prompt a sub-agent to do a task for you.\n\n\
             Agent name: {name}\n\
             Agent description: {desc}\n\
             Agent system prompt: {preamble}",
            name = self.label,
            desc = self.inner.description.clone().unwrap_or_default(),
            preamble = self.inner.preamble.clone().unwrap_or_default(),
        );
        ToolDefinition {
            name: self.name(),
            description,
            parameters: serde_json::to_value(schemars::schema_for!(AgentToolArgs))
                .expect("schema conversion should not fail"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("\n  ┌─ 🤖 Sub-agent [{label}] invoked", label = self.label);

        let result = self.inner.prompt(args.prompt).await;
        match &result {
            Ok(response) => {
                let preview = truncate(response, 200);
                println!("  │  Response: {preview}");
                println!("  └─ ✅ Sub-agent [{label}] done\n", label = self.label);
            }
            Err(e) => {
                println!(
                    "  └─ ❌ Sub-agent [{label}] failed: {e}\n",
                    label = self.label
                );
            }
        }
        result
    }

    fn name(&self) -> String {
        format!("agent_{}", self.label)
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    let truncated = truncated.replace('\n', " ");
    if s.chars().count() > max_chars {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::Tool as McpTool;
    use serde_json::json;
    use std::borrow::Cow;
    use std::sync::Arc;

    fn make_tool(name: &str) -> McpTool {
        McpTool {
            name: Cow::Owned(name.to_string()),
            title: None,
            description: Some(Cow::Owned(format!("{name} tool"))),
            input_schema: Arc::new(serde_json::from_value(json!({"type": "object"})).unwrap()),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        }
    }

    fn tool_names(tools: &[McpTool]) -> Vec<String> {
        tools.iter().map(|t| t.name.to_string()).collect()
    }

    // ── filter_tools ──

    #[test]
    fn filter_tools_keeps_only_permitted() {
        let tools = vec![
            make_tool("read_file"),
            make_tool("write_file"),
            make_tool("list_directory"),
        ];
        let permitted = vec!["read_file".to_string(), "list_directory".to_string()];

        let filtered = filter_tools(tools, &permitted);
        assert_eq!(tool_names(&filtered), vec!["read_file", "list_directory"]);
    }

    #[test]
    fn filter_tools_empty_permitted_returns_nothing() {
        let tools = vec![make_tool("read_file"), make_tool("write_file")];
        let filtered = filter_tools(tools, &[]);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_tools_no_matching_tools_returns_empty() {
        let tools = vec![make_tool("read_file")];
        let permitted = vec!["write_file".to_string()];
        let filtered = filter_tools(tools, &permitted);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_tools_preserves_tool_metadata() {
        let tools = vec![make_tool("read_file")];
        let permitted = vec!["read_file".to_string()];
        let filtered = filter_tools(tools, &permitted);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].description.as_deref(), Some("read_file tool"));
    }

    // ── config-driven filtering (unified for all agents) ──

    #[test]
    fn agent_config_filters_tools_correctly() {
        let config = AgentConfig {
            name: "orchestrator".into(),
            description: String::new(),
            preamble: String::new(),
            authz_mode: AuthzMode::default(),
            permitted_actions: vec!["read_file".into(), "list_directory".into()],
        };

        let all_tools = vec![
            make_tool("read_file"),
            make_tool("write_file"),
            make_tool("list_directory"),
            make_tool("edit_file"),
        ];

        let filtered = filter_tools(all_tools, &config.permitted_actions);
        assert_eq!(tool_names(&filtered), vec!["read_file", "list_directory"]);
    }

    #[test]
    fn same_filter_works_for_sub_agent() {
        let config = AgentConfig {
            name: "researcher".into(),
            description: "test".into(),
            preamble: String::new(),
            authz_mode: AuthzMode::default(),
            permitted_actions: vec!["read_file".into(), "search_files".into()],
        };

        let all_tools = vec![
            make_tool("read_file"),
            make_tool("write_file"),
            make_tool("search_files"),
            make_tool("edit_file"),
        ];

        let filtered = filter_tools(all_tools, &config.permitted_actions);
        assert_eq!(tool_names(&filtered), vec!["read_file", "search_files"]);
    }

    #[test]
    fn agent_with_empty_permitted_sees_no_tools() {
        let config = AgentConfig {
            name: "media_creator".into(),
            description: "test".into(),
            preamble: String::new(),
            authz_mode: AuthzMode::default(),
            permitted_actions: vec![],
        };

        let all_tools = vec![make_tool("read_file"), make_tool("write_file")];
        let filtered = filter_tools(all_tools, &config.permitted_actions);
        assert!(filtered.is_empty());
    }

    #[test]
    fn agent_cannot_see_tools_not_in_permitted() {
        let config = AgentConfig {
            name: "researcher".into(),
            description: "test".into(),
            preamble: String::new(),
            authz_mode: AuthzMode::default(),
            permitted_actions: vec!["read_file".into()],
        };

        let all_tools = vec![
            make_tool("read_file"),
            make_tool("write_file"),
            make_tool("publish_instagram_live"),
        ];

        let filtered = filter_tools(all_tools, &config.permitted_actions);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name.as_ref(), "read_file");
    }

    // ── authz_mode ──

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
}
