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
        approver_hmac_key: std::env::var("APPROVER_HMAC_KEY")
            .unwrap_or_default(),
        local_key_id: std::env::var("CLAWDIA_LOCAL_KEY_ID")
            .unwrap_or_else(|_| "local".to_string()),
        local_roles: std::env::var("CLAWDIA_LOCAL_ROLES")
            .unwrap_or_else(|_| "campaign_owner".to_string()),
        log_dir: std::env::var("CLAWDIA_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./logs")),
    };

    if chat_args.approver_hmac_key.is_empty() {
        anyhow::bail!(
            "APPROVER_HMAC_KEY must be set (e.g. via .env or environment). \
             Run with --authz-backend yaml --approver-hmac-key dummy if testing."
        );
    }

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
