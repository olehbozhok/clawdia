use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

impl From<LogLevel> for tracing_subscriber::filter::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Self::TRACE,
            LogLevel::Debug => Self::DEBUG,
            LogLevel::Info => Self::INFO,
            LogLevel::Warn => Self::WARN,
            LogLevel::Error => Self::ERROR,
            LogLevel::Off => Self::OFF,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AuthzBackendChoice {
    /// Cedar/Cedarling policy engine (default, fails closed if not loadable).
    Cedarling,
    /// Flat YAML permitted_actions list. Development/debug only.
    Yaml,
}

#[derive(Parser)]
#[command(name = "clawdia")]
#[command(about = "Clawdia Schiffer — policy-governed AI agent CLI")]
pub struct Cli {
    /// Log level
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    pub log_level: LogLevel,

    /// Path to MCP servers config
    #[arg(long, env = "MCP_CONFIG", default_value = "config/mcp_servers.yaml")]
    pub mcp_config: PathBuf,

    /// Path to agents config
    #[arg(long, env = "AGENTS_CONFIG", default_value = "config/agents.yaml")]
    pub agents_config: PathBuf,

    /// DeepSeek API key
    #[arg(long, env = "DEEPSEEK_API_KEY", hide_env_values = true)]
    pub api_key: String,

    /// DeepSeek model name
    #[arg(long, env = "DEEPSEEK_MODEL", default_value = "deepseek-chat")]
    pub model: String,

    /// Path to Cedar policy store directory
    #[arg(
        long,
        env = "CLAWDIA_POLICY_STORE_PATH",
        default_value = "config/policies"
    )]
    pub policy_store: PathBuf,

    /// Authorization backend. `cedarling` is the secure default; `yaml`
    /// is a developer opt-in that disables policy enforcement.
    #[arg(
        long,
        env = "CLAWDIA_AUTHZ_BACKEND",
        value_enum,
        default_value = "cedarling"
    )]
    pub authz_backend: AuthzBackendChoice,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start interactive chat with the orchestrator agent
    Chat,
    /// List all available tools from connected MCP servers
    Tools {
        /// Output full tool definitions as JSON (includes input schemas)
        #[arg(long)]
        json: bool,
    },
    /// List configured agents and their permitted actions
    Agents,
    /// Auto-generate a Cedarling policy store from MCP tools and agent roles.
    AdvisorGenerate {
        /// Output directory for the generated policy store
        #[arg(long, default_value = "config/policies")]
        output: PathBuf,

        /// Cedar policy store ID (hex 8-64 chars). If omitted, a random hex ID is generated.
        #[arg(long)]
        policy_store_id: Option<String>,

        /// System entity ID
        #[arg(long, default_value = "clawdia")]
        system_entity_id: String,
    },
}
