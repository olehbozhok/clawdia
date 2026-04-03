use std::collections::HashMap;
use std::path::Path;

use rmcp::ServiceExt;
use rmcp::model::Tool;
use rmcp::service::{RunningService, ServerSink};
use rmcp::transport::TokioChildProcess;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransportConfig, StreamableHttpClientWorker,
};
use serde::Deserialize;
use tokio::process::Command;

// ── Config types ──

#[derive(Debug, Deserialize)]
pub struct Config {
    pub servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "transport")]
pub enum McpServerConfig {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

// ── MCP connection ──

pub type McpRunning = RunningService<rmcp::RoleClient, ()>;

async fn connect_server(cfg: &McpServerConfig) -> Result<McpRunning, Box<dyn std::error::Error>> {
    match cfg {
        McpServerConfig::Stdio { command, args, env } => {
            let mut cmd = Command::new(command);
            cmd.args(args);
            for (k, v) in env {
                cmd.env(k, v);
            }
            let transport = TokioChildProcess::new(cmd)?;
            Ok(().serve(transport).await?)
        }
        McpServerConfig::Http { url, headers } => {
            let mut config = StreamableHttpClientTransportConfig::with_uri(url.as_str());
            if !headers.is_empty() {
                let map = headers
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            k.parse::<http::HeaderName>()?,
                            v.parse::<http::HeaderValue>()?,
                        ))
                    })
                    .collect::<Result<HashMap<_, _>, Box<dyn std::error::Error>>>()?;
                config.custom_headers = map;
            }
            let worker =
                StreamableHttpClientWorker::<reqwest::Client>::new(Default::default(), config);
            Ok(().serve(worker).await?)
        }
    }
}

/// Load MCP config and connect to all servers.
/// Returns collected (ServerSink, Tools) pairs and running services to keep alive.
pub async fn connect_all(
    config_path: &Path,
) -> Result<(Vec<(ServerSink, Vec<Tool>)>, Vec<McpRunning>), Box<dyn std::error::Error>> {
    let config: Config = serde_yaml::from_str(&std::fs::read_to_string(config_path)?)?;

    let mut server_tools: Vec<(ServerSink, Vec<Tool>)> = Vec::new();
    let mut running_services: Vec<McpRunning> = Vec::new();

    for (_name, server_cfg) in &config.servers {
        let service = connect_server(server_cfg).await?;

        let tools_response = service.peer().list_tools(Default::default()).await?;
        // print tools info

        // println!("\n=== {_name} ({} tools) ===", tools_response.tools.len());
        // for tool in &tools_response.tools {
        //     println!("\n  {}:", tool.name);
        //     println!(
        //         "    description: {}",
        //         tool.description.as_deref().unwrap_or("")
        //     );
        //     println!(
        //         "    inputSchema: {}",
        //         serde_json::to_string_pretty(&tool.input_schema)?
        //     );
        // }

        server_tools.push((service.peer().clone(), tools_response.tools));
        running_services.push(service);
    }

    Ok((server_tools, running_services))
}
