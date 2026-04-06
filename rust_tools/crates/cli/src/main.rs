mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clawdia")]
#[command(about = "Clawdia Schiffer — policy-governed AI agent CLI")]
struct Cli {
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

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start interactive chat with the orchestrator agent
    Chat,
    /// List all available tools from connected MCP servers
    Tools,
    /// List configured agents and their permitted actions
    Agents,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Command::Chat => {
            commands::chat(
                &cli.mcp_config,
                &cli.agents_config,
                &cli.api_key,
                &cli.model,
            )
            .await
        }
        Command::Tools => commands::tools(&cli.mcp_config).await,
        Command::Agents => commands::agents(&cli.agents_config),
    }
}
