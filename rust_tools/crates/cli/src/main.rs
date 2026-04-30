use clap::Parser;
use cli::{commands, Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
                cli.authz_backend,
            )
            .await
        }
        Command::Tools { json } => commands::tools(&cli.mcp_config, json).await,
        Command::Agents => commands::agents(&cli.agents_config),
        Command::AdvisorGenerate {
            output,
            policy_store_id,
            system_entity_id,
        } => {
            commands::advisor_generate(
                &cli.mcp_config,
                &cli.agents_config,
                &output,
                &cli.api_key,
                &cli.model,
                policy_store_id.as_deref(),
                &system_entity_id,
            )
            .await
        }
    }
}
