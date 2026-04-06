use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::completion::{CompletionModel, Prompt, PromptError, ToolDefinition};
use rig::tool::Tool;

use crate::authz_hook::AuthzHook;
use crate::mcp::McpServer;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools::authz::{AuditEntry, AuthorizationDecision, Principal};

// ── Config types ──

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub orchestrator: AgentConfig,
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub preamble: String,
    #[serde(default)]
    pub permitted_actions: Vec<String>,
}

pub fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

// ── Tool filtering ──

/// Filter MCP tools to only include those in the permitted actions list.
pub fn filter_tools(
    tools: Vec<rmcp::model::Tool>,
    permitted: &[String],
) -> Vec<rmcp::model::Tool> {
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
    );

    let permitted = &config.permitted_actions;

    let base = client
        .agent(model)
        .preamble(&config.preamble)
        .name(&config.name)
        .description(&config.description)
        .default_max_turns(20)
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

/// Build the orchestrator: same as any agent, plus all sub-agents attached as tools.
/// Sub-agents are always available to the orchestrator (no permission needed).
pub fn build_orchestrator<C: CompletionClient + 'static>(
    client: &C,
    model: &str,
    config: &Config,
    servers: Vec<McpServer>,
    audit_log: &AuditLog,
) -> Agent<C::CompletionModel, AuthzHook> {
    let principal = Principal {
        id: config.orchestrator.name.clone(),
        principal_type: config.orchestrator.name.clone(),
        delegation_record_id: None,
    };

    let hook = AuthzHook::new(
        principal,
        config.orchestrator.permitted_actions.clone(),
        audit_log.clone(),
    );

    let permitted = &config.orchestrator.permitted_actions;

    let base = client
        .agent(model)
        .preamble(&config.orchestrator.preamble)
        .name(&config.orchestrator.name)
        .description(&config.orchestrator.description)
        .default_max_turns(20)
        .hook(hook);

    // Attach filtered MCP tools
    let mut groups = servers.iter();
    let first = groups
        .next()
        .expect("At least one MCP server must be configured");

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

    // All sub-agents are always available to the orchestrator
    for agent_cfg in config.agents.values() {
        let inner = build_agent(client, model, agent_cfg, &servers, audit_log);
        let sub_agent = VerboseAgent {
            label: agent_cfg.name.clone(),
            inner,
        };
        agent_builder = agent_builder.tool(sub_agent);
    }

    agent_builder.build()
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
        let decision_icon = match entry.decision {
            AuthorizationDecision::Allow => "✅",
            AuthorizationDecision::Deny => "🚫",
        };
        println!(
            "  │  {icon} [{principal}] {action} on {resource} → {decision:?}{reason}",
            icon = decision_icon,
            principal = entry.principal_id,
            action = entry.action,
            resource = entry.resource,
            decision = entry.decision,
            reason = entry
                .denial_reason
                .as_deref()
                .map(|r| format!(" ({r})"))
                .unwrap_or_default(),
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
        println!(
            "\n  ┌─ 🤖 Sub-agent [{label}] invoked",
            label = self.label
        );

        let result = self.inner.prompt(args.prompt).await;
        match &result {
            Ok(response) => {
                let preview = truncate(response, 200);
                println!("  │  Response: {preview}");
                println!(
                    "  └─ ✅ Sub-agent [{label}] done\n",
                    label = self.label
                );
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
        self.inner
            .name
            .clone()
            .unwrap_or_else(|| Self::NAME.to_string())
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
            permitted_actions: vec![
                "researcher".into(),
                "read_file".into(),
                "list_directory".into(),
            ],
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
}
