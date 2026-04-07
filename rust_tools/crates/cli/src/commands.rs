use std::io::{self, BufRead, Write};
use std::path::Path;

use rig::completion::Prompt;
use rig::providers::deepseek;
use runtime::agents::{self, AuditLog};

pub async fn chat(
    mcp_config: &Path,
    agents_config: &Path,
    api_key: &str,
    model_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = agents::load_config(agents_config)?;
    let (servers, running_services) = runtime::mcp::connect_all(mcp_config).await?;

    let audit_log = AuditLog::new();
    let client = deepseek::Client::new(api_key)?;

    let agent = agents::build_orchestrator(&client, model_name, &config, servers, &audit_log);

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

pub async fn tools(mcp_config: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (servers, running_services) = runtime::mcp::connect_all(mcp_config).await?;

    if json {
        let mut output = serde_json::Map::new();
        for server in &servers {
            let tools: Vec<serde_json::Value> = server
                .tools
                .iter()
                .map(|t| serde_json::to_value(t).unwrap_or_default())
                .collect();
            output.insert(server.name.clone(), serde_json::Value::Array(tools));
        }
        // Write JSON to stdout; tracing goes to stderr via subscriber config
        print!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Object(output))?
        );
    } else {
        println!("Available tools from MCP servers:\n");
        for server in &servers {
            println!("  [{name}]", name = server.name);
            for tool in &server.tools {
                let desc = tool.description.as_deref().unwrap_or("(no description)");
                println!("    - {name}", name = tool.name);
                println!("      {desc}\n");
            }
        }
        println!("Copy tool names into agents.yaml → permitted_actions to grant access.");
    }

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
