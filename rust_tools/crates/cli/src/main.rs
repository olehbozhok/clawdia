mod cli_args;
mod commands;
mod tui;

use clap::Parser;
use cli_args::{ChatArgs, Cli, Command};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    let chat_args = ChatArgs {
        mcp_config: cli.mcp_config.clone(),
        agents_config: cli.agents_config.clone(),
        api_key: cli.api_key.clone(),
        model: cli.model.clone(),
        policy_store: cli.policy_store.clone(),
        authz_backend: cli.authz_backend,
        approver_hmac_key: String::new(),
        local_key_id: "local".to_string(),
        local_roles: "campaign_owner".to_string(),
        log_dir: PathBuf::from("./logs"),
    };

    match cli.command {
        Command::Chat => commands::chat(&chat_args).await,
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
