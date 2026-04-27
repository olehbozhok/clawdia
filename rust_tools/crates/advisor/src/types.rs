use serde::{Deserialize, Serialize};

/// User-supplied description of a single agent role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSpec {
    pub name: String,
    pub description: String,
    pub permitted_tools: Vec<String>,
}

/// A tool discovered via MCP.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredTool {
    pub server: String,
    pub name: String,
    pub description: String,
}

/// Top-level input to `advisor::run`.
#[derive(Debug, Clone)]
pub struct AdvisorInput {
    pub agents: Vec<AgentSpec>,
    pub tools: Vec<DiscoveredTool>,
    pub policy_store_id: String,
    pub system_entity_id: String,
    pub domain_hint: Option<String>,
}

/// Generated artifacts (in-memory) before being written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifacts {
    pub schema: String,
    /// Map of agent name → Cedar policy file contents.
    pub policies: std::collections::BTreeMap<String, String>,
    pub agents_json: String,
    pub system_json: String,
    pub metadata_json: String,
}

/// Result of a successful advisor run.
#[derive(Debug, Clone)]
pub struct AdvisorOutput {
    pub output_dir: std::path::PathBuf,
    pub artifacts: Artifacts,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_spec_serde_roundtrip() {
        let spec = AgentSpec {
            name: "researcher".to_string(),
            description: "Searches the web".to_string(),
            permitted_tools: vec!["search".to_string(), "fetch_content".to_string()],
        };
        let json = serde_json::to_string(&spec).unwrap();
        let back: AgentSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn discovered_tool_serde_roundtrip() {
        let t = DiscoveredTool {
            server: "campaign-doc".to_string(),
            name: "doc_create".to_string(),
            description: "Create a campaign".to_string(),
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: DiscoveredTool = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }
}
