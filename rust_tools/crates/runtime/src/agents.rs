use std::collections::HashMap;
use std::path::Path;

use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::completion::{CompletionModel, Prompt, PromptError, ToolDefinition};
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ── Config types ──

#[derive(Debug, Deserialize)]
pub struct Config {
    pub orchestrator: OrchestratorConfig,
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Deserialize)]
pub struct OrchestratorConfig {
    pub preamble: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub preamble: String,
}

pub fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

// ── Verbose wrapper ──

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentToolArgs {
    prompt: String,
}

/// Wraps an `Agent` to print status messages when the sub-agent is called.
pub struct VerboseAgent<M: CompletionModel> {
    inner: Agent<M>,
    label: String,
}

impl<M: CompletionModel + 'static> Tool for VerboseAgent<M> {
    const NAME: &'static str = "agent_tool";

    type Error = PromptError;
    type Args = AgentToolArgs;
    type Output = String;

    async fn definition(&self, prompt: String) -> ToolDefinition {
        self.inner.definition(prompt).await
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("\n  ┌─ 🤖 Sub-agent [{label}] started", label = self.label);
        let result = self.inner.prompt(args.prompt).await;
        match &result {
            Ok(response) => {
                let preview = truncate(response, 200);
                println!("  │  Response: {preview}");
                println!("  └─ ✅ Sub-agent [{label}] done\n", label = self.label);
            }
            Err(e) => {
                println!("  └─ ❌ Sub-agent [{label}] failed: {e}\n", label = self.label);
            }
        }
        result
    }

    fn name(&self) -> String {
        self.inner.name()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.replace('\n', " ")
    } else {
        format!("{}…", s[..max].replace('\n', " "))
    }
}

// ── Builder ──

pub fn build_sub_agent<C: CompletionClient>(
    client: &C,
    model: &str,
    config: &AgentConfig,
) -> VerboseAgent<C::CompletionModel> {
    let inner = client
        .agent(model)
        .preamble(&config.preamble)
        .name(&config.name)
        .description(&config.description)
        .build();

    VerboseAgent {
        label: config.name.clone(),
        inner,
    }
}
