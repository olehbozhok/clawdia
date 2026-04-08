mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
enum LogLevel {
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

#[derive(Parser)]
#[command(name = "clawdia")]
#[command(about = "Clawdia Schiffer — policy-governed AI agent CLI")]
struct Cli {
    /// Log level
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    log_level: LogLevel,

    /// Path to MCP servers config
    #[arg(long, env = "MCP_CONFIG", default_value = "config/mcp_servers.yaml")]
    mcp_config: PathBuf,

    /// Path to agents config
    #[arg(long, env = "AGENTS_CONFIG", default_value = "config/agents.yaml")]
    agents_config: PathBuf,

    /// DeepSeek API key
    #[arg(long, env = "DEEPSEEK_API_KEY", hide_env_values = true)]
    api_key: String,

    /// DeepSeek model name
    #[arg(long, env = "DEEPSEEK_MODEL", default_value = "deepseek-chat")]
    model: String,

    /// Path to Cedar policy store directory
    #[arg(
        long,
        env = "CLAWDIA_POLICY_STORE_PATH",
        default_value = "config/policies"
    )]
    policy_store: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    let level: tracing_subscriber::filter::LevelFilter = cli.log_level.into();
    tracing_subscriber::fmt().with_max_level(level).init();

    match cli.command {
        Command::Chat => {
            commands::chat(
                &cli.mcp_config,
                &cli.agents_config,
                &cli.api_key,
                &cli.model,
                &cli.policy_store,
            )
            .await
        }
        Command::Tools { json } => commands::tools(&cli.mcp_config, json).await,
        Command::Agents => commands::agents(&cli.agents_config),
    }
}
