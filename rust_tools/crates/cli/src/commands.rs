use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;

use rig::completion::Prompt;
use rig::providers::deepseek;
use runtime::agents::{self, AuditLog};
use runtime::authz_hook::AuthzBackend;
use runtime::cedar_authz::CedarAuthz;
use runtime::policy_prompt;

pub async fn chat(
    mcp_config: &Path,
    agents_config: &Path,
    api_key: &str,
    model_name: &str,
    policy_store: &Path,
    authz_backend: crate::cli_args::AuthzBackendChoice,
) -> anyhow::Result<()> {
    let config = agents::load_config(agents_config).map_err(|e| anyhow::anyhow!("{e}"))?;
    let (servers, running_services) =
        runtime::mcp::connect_all(mcp_config).await.map_err(|e| anyhow::anyhow!("{e}"))?;

    let audit_log = AuditLog::new();
    let client = deepseek::Client::new(api_key).map_err(|e| anyhow::anyhow!("{e}"))?;

    let cedar = match authz_backend {
        crate::cli_args::AuthzBackendChoice::Cedarling => {
            let cedar = CedarAuthz::from_directory(policy_store)
                .await
                .map_err(|e| anyhow::anyhow!(
                    "Cedarling backend selected but failed to load policy store \
                     at {}: {e}. Re-run with --authz-backend yaml only for \
                     development/debugging.",
                    policy_store.display(),
                ))?;
            tracing::info!(
                "Cedar policy engine loaded from {}",
                policy_store.display()
            );
            Some(Arc::new(cedar))
        }
        crate::cli_args::AuthzBackendChoice::Yaml => {
            tracing::warn!(
                "YAML authorization backend selected — Cedar policies are NOT \
                 enforced. This mode is for development/debugging only."
            );
            None
        }
    };

    let backend = match &cedar {
        Some(c) => AuthzBackend::Cedar(c.clone()),
        None => AuthzBackend::Yaml,
    };

    let agent_permissions = if policy_store.exists() {
        policy_prompt::load_permissions_from_policies(policy_store).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let agent = agents::build_orchestrator(
        &client,
        model_name,
        &config,
        servers,
        &audit_log,
        backend,
        &agent_permissions,
    );

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
        svc.cancel().await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

pub async fn tools(mcp_config: &Path, json: bool) -> anyhow::Result<()> {
    let (servers, running_services) =
        runtime::mcp::connect_all(mcp_config).await.map_err(|e| anyhow::anyhow!("{e}"))?;

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
        svc.cancel().await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

pub async fn advisor_generate(
    mcp_config: &Path,
    agents_config: &Path,
    output: &Path,
    api_key: &str,
    model: &str,
    policy_store_id: Option<&str>,
    system_entity_id: &str,
) -> anyhow::Result<()> {
    let agents_cfg = runtime::agents::load_config(agents_config).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut agents = Vec::new();
    for (name, agent) in &agents_cfg.agents {
        agents.push(advisor::AgentSpec {
            name: name.clone(),
            description: agent.description.clone(),
            permitted_tools: agent.permitted_actions.clone(),
        });
    }

    let tools = advisor::discovery::discover(mcp_config).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Discovered {} tools across MCP servers.", tools.len());

    let client = advisor::RigClient::new(api_key, model).map_err(|e| anyhow::anyhow!("{e}"))?;

    let policy_store_id = match policy_store_id {
        Some(id) => id.to_string(),
        None => random_hex_id(),
    };

    let input = advisor::AdvisorInput {
        agents,
        tools,
        policy_store_id,
        system_entity_id: system_entity_id.to_string(),
        domain_hint: None,
    };

    let out = advisor::run(&client, input, output.to_path_buf()).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Wrote policy store to {}", out.output_dir.display());
    Ok(())
}

fn random_hex_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:024x}", nanos)
}

pub fn agents(agents_config: &Path) -> anyhow::Result<()> {
    let config = agents::load_config(agents_config).map_err(|e| anyhow::anyhow!("{e}"))?;

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
