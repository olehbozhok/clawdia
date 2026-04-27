use serde::{Deserialize, Serialize};

/// Input provided to the advisor to generate a policy store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvisorInput {}

/// Output produced by the advisor containing generated artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvisorOutput {
    pub artifacts: Artifacts,
}

/// Specification for an agent role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {}

/// Generated Cedar policy store artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifacts {}

/// A tool discovered from an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTool {}
