use std::io::{self, BufRead, Write};
use std::path::Path;

use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::deepseek;
use runtime::agents::{self, AuditLog};
use runtime::authz_hook::AuthzHook;
use tools::authz::{Principal, PrincipalType};

pub async fn chat(
    mcp_config: &Path,
    agents_config: &Path,
    api_key: &str,
    model_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let agents_config = agents::load_config(agents_config)?;
    let (server_tools, running_services) = runtime::mcp::connect_all(mcp_config).await?;

    let audit_log = AuditLog::new();
    let client = deepseek::Client::new(api_key)?;

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

    let mut groups = server_tools.into_iter();
    let (first_peer, first_tools) = groups
        .next()
        .expect("At least one MCP server must be configured");

    let mut agent_builder = client
        .agent(model_name)
        .preamble(&agents_config.orchestrator.preamble)
        .default_max_turns(20)
        .hook(orchestrator_hook)
        .rmcp_tools(first_tools, first_peer);

    for (peer, tools) in groups {
        agent_builder = agent_builder.rmcp_tools(tools, peer);
    }

    for agent_cfg in agents_config.agents.values() {
        let sub_agent = agents::build_sub_agent(&client, model_name, agent_cfg, &audit_log);
        agent_builder = agent_builder.tool(sub_agent);
    }

    let agent = agent_builder.build();

    println!("\nClawdia Schiffer — interactive chat (type 'quit' to exit)");

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

pub async fn tools(mcp_config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (server_tools, running_services) = runtime::mcp::connect_all(mcp_config).await?;

    println!("Available tools from MCP servers:\n");

    for (_peer, tools) in &server_tools {
        for tool in tools {
            let desc = tool
                .description
                .as_deref()
                .unwrap_or("(no description)");
            println!("  - {name}", name = tool.name);
            println!("    {desc}\n");
        }
    }

    println!("Copy tool names into agents.yaml → permitted_actions to grant access.");

    for svc in running_services {
        svc.cancel().await?;
    }
    Ok(())
}

pub fn agents(agents_config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let config = agents::load_config(agents_config)?;

    println!("Orchestrator:");
    if config.orchestrator.permitted_actions.is_empty() {
        println!("  permitted_actions: (none — all tools denied)");
    } else {
        println!("  permitted_actions:");
        for action in &config.orchestrator.permitted_actions {
            println!("    - {action}");
        }
    }

    println!("\nSub-agents:\n");
    for (key, agent) in &config.agents {
        println!("  {key}:");
        println!("    name: {}", agent.name);
        println!("    description: {}", agent.description);
        if agent.permitted_actions.is_empty() {
            println!("    permitted_actions: (none — all tools denied)");
        } else {
            println!("    permitted_actions:");
            for action in &agent.permitted_actions {
                println!("      - {action}");
            }
        }
        println!();
    }

    Ok(())
}
