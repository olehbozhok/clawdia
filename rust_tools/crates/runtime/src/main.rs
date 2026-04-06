mod agents;
mod authz_hook;
mod mcp;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::deepseek;
use tools::authz::{Principal, PrincipalType};

use crate::authz_hook::AuthzHook;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    // Read all environment variables upfront
    let mcp_config_path = std::env::var("MCP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/mcp_servers.yaml"));
    let agents_config_path = std::env::var("AGENTS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/agents.yaml"));
    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");
    let model_name = std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    // Load configs
    let agents_config = agents::load_config(&agents_config_path)?;
    let (server_tools, running_services) = mcp::connect_all(&mcp_config_path).await?;

    // Shared audit log
    let audit_log = agents::AuditLog::new();

    // Initialize DeepSeek client
    let client = deepseek::Client::new(&api_key)?;

    // AuthzHook for the orchestrator — tracks all tool calls from MainAgent
    let orchestrator_principal = Principal {
        id: "orchestrator".to_string(),
        principal_type: PrincipalType::MainAgent,
        delegation_record_id: None,
    };
    let orchestrator_hook = AuthzHook::new(
        orchestrator_principal,
        agents_config.orchestrator.permitted_actions.clone(),
        audit_log.clone(),
    );

    // Build orchestrator agent with MCP tools and AuthzHook
    let mut groups = server_tools.into_iter();
    let (first_peer, first_tools) = groups
        .next()
        .expect("At least one MCP server must be configured");

    let mut agent_builder = client
        .agent(&model_name)
        .preamble(&agents_config.orchestrator.preamble)
        .default_max_turns(20)
        .hook(orchestrator_hook)
        .rmcp_tools(first_tools, first_peer);

    for (peer, tools) in groups {
        agent_builder = agent_builder.rmcp_tools(tools, peer);
    }

    // Add sub-agents as tools
    for agent_cfg in agents_config.agents.values() {
        let sub_agent = agents::build_sub_agent(&client, &model_name, agent_cfg, &audit_log);
        agent_builder = agent_builder.tool(sub_agent);
    }

    let agent = agent_builder.build();

    // Interactive chat loop
    println!("\nChat with DeepSeek + MCP (type 'quit' to exit)");

    let stdin = io::stdin();
    loop {
        print!("\nYou: ");
        io::stdout().flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let prompt = input.trim();

        if prompt.is_empty() || prompt == "quit" || prompt == "exit" {
            break;
        }

        match agent.prompt(prompt).await {
            Ok(response) => println!("\nAssistant: {response}"),
            Err(e) => eprintln!("\nError: {e}"),
        }
    }

    for svc in running_services {
        svc.cancel().await?;
    }
    Ok(())
}
