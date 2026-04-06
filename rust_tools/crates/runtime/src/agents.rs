use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::completion::{CompletionModel, Prompt, PromptError, ToolDefinition};
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools::authz::{AuditEntry, AuthorizationDecision, Principal, PrincipalType};

use crate::authz_hook::AuthzHook;

// ── Config types ──

#[derive(Debug, Deserialize)]
pub struct Config {
    pub orchestrator: OrchestratorConfig,
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Deserialize)]
pub struct OrchestratorConfig {
    pub preamble: String,
    #[serde(default)]
    pub permitted_actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub preamble: String,
    #[serde(default)]
    pub permitted_actions: Vec<String>,
}

pub fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
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
/// Authorization is handled by `AuthzHook` attached to the parent agent.
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

// ── Builder ──

pub fn build_sub_agent<C: CompletionClient>(
    client: &C,
    model: &str,
    config: &AgentConfig,
    audit_log: &AuditLog,
) -> VerboseAgent<C::CompletionModel> {
    let principal_type = match config.name.as_str() {
        "researcher" => PrincipalType::ResearchSubAgent,
        "media_creator" => PrincipalType::MediaSubAgent,
        _ => PrincipalType::MainAgent,
    };

    let principal = Principal {
        id: config.name.clone(),
        principal_type,
        delegation_record_id: None,
    };

    let hook = AuthzHook::new(
        principal,
        config.permitted_actions.clone(),
        audit_log.clone(),
    );

    let inner = client
        .agent(model)
        .preamble(&config.preamble)
        .name(&config.name)
        .description(&config.description)
        .hook(hook)
        .build();

    VerboseAgent {
        label: config.name.clone(),
        inner,
    }
}
