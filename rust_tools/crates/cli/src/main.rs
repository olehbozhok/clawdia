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

    // Suppress tracing output when JSON output is requested
    let json_mode = matches!(cli.command, Command::Tools { json: true });
    if json_mode {
        tracing_subscriber::fmt()
            .with_max_level(tracing_subscriber::filter::LevelFilter::ERROR)
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

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
        Command::Tools { json } => commands::tools(&cli.mcp_config, json).await,
        Command::Agents => commands::agents(&cli.agents_config),
    }
}
