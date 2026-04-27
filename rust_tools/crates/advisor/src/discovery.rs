//! MCP tool discovery — connects to all configured MCP servers and flattens
//! their tool catalogs into `Vec<DiscoveredTool>`.

use std::path::Path;

use anyhow::Result;

use crate::types::DiscoveredTool;

/// Connect to all MCP servers in `mcp_config`, enumerate their tools, then
/// disconnect. Returns the flattened catalog.
pub async fn discover(mcp_config: &Path) -> Result<Vec<DiscoveredTool>> {
    let (servers, running) = runtime::mcp::connect_all(mcp_config)
        .await
        .map_err(|e| anyhow::anyhow!("MCP connect_all failed: {e}"))?;

    let mut tools = Vec::new();
    for server in &servers {
        for t in &server.tools {
            tools.push(DiscoveredTool {
                server: server.name.clone(),
                name: t.name.to_string(),
                description: t
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }

    for svc in running {
        let _ = svc.cancel().await;
    }

    Ok(tools)
}
