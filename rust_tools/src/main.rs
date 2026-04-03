mod mcp;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::deepseek;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    // Read all environment variables upfront
    let mcp_config_path = std::env::var("MCP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("mcp_servers.yaml"));
    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");
    let model_name =
        std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    // Connect all MCP servers
    let (server_tools, running_services) = mcp::connect_all(&mcp_config_path).await?;

    // Initialize DeepSeek client
    let client = deepseek::Client::new(&api_key)?;

    // Build agent with all MCP tools
    let mut groups = server_tools.into_iter();
    let (first_peer, first_tools) = groups
        .next()
        .expect("At least one MCP server must be configured");

    let mut agent_builder = client
        .agent(&model_name)
        .preamble("You are a helpful assistant with access to MCP tools.")
        .rmcp_tools(first_tools, first_peer);

    for (peer, tools) in groups {
        agent_builder = agent_builder.rmcp_tools(tools, peer);
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
