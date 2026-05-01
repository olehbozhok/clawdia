//! Prompt templates for LLM-driven Cedar artifact generation.
//!
//! Two prompts are needed: one for the schema (single call), one for each
//! agent's policy file (one call per agent).

use crate::types::{AgentSpec, DiscoveredTool};

const SCHEMA_SYSTEM: &str = "You generate Cedar policy schemas in the AgentPolicy namespace. \
Output ONLY the schema content — no markdown fences, no commentary. \
Each tool becomes one Cedar action whose name is the tool name verbatim. \
Principal is always `Agent`, resource is always `System`. \
The Agent entity has a single attribute `agent_type: String`. \
The context shape can vary per action; use `{ requested_domain?: String }` for tools \
whose name starts with `fetch_` or `chrome_`, `{ campaign_id?: String }` for tools whose \
name starts with `doc_` (excluding `doc_list`, `doc_status`, `doc_create`), and `{}` otherwise.";

pub fn schema_prompt(tools: &[DiscoveredTool]) -> (String, String) {
    let mut user = String::from("Generate a Cedar schema for these tools:\n\n");
    for t in tools {
        user.push_str(&format!("- {} \u{2014} {}\n", t.name, t.description));
    }
    user.push_str("\nReturn only the schema, starting with `namespace AgentPolicy {`.\n");
    (SCHEMA_SYSTEM.to_string(), user)
}

const POLICY_SYSTEM: &str = "You generate Cedar policies for one agent in the AgentPolicy namespace. \
Output ONLY the policy file content — no markdown fences, no commentary. \
Each permitted tool becomes one `permit` policy. \
Each policy MUST have an `@description(\"...\")` and an `@tool(\"<tool_name>\")` annotation. \
The principal is `AgentPolicy::Agent::\"<agent_name>\"`, the action is \
`AgentPolicy::Action::\"<tool_name>\"`, and the resource is \
`AgentPolicy::System::\"<system_id>\"`. Multiple policies in one file are OK; separate by blank lines.";

pub fn policy_prompt(
    agent: &AgentSpec,
    system_entity_id: &str,
    tools: &[DiscoveredTool],
) -> (String, String) {
    let mut user = format!(
        "Agent name: {}\nAgent description: {}\nSystem entity id: {}\n\nPermitted tools:\n",
        agent.name, agent.description, system_entity_id
    );
    for tool_name in &agent.permitted_tools {
        let desc = tools
            .iter()
            .find(|t| &t.name == tool_name)
            .map(|t| t.description.as_str())
            .unwrap_or("");
        user.push_str(&format!("- {tool_name} \u{2014} {desc}\n"));
    }
    user.push_str("\nReturn only the .cedar file content.\n");
    (POLICY_SYSTEM.to_string(), user)
}

/// Append validation errors so the LLM can correct its previous output.
pub fn retry_prompt(prev_output: &str, errors: &str) -> String {
    format!(
        "Your previous output failed validation. Output again, fixing the errors below.\n\n\
         === Previous output ===\n{prev_output}\n\n\
         === Validation errors ===\n{errors}\n\n\
         Output only the corrected content."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_prompt_lists_tools() {
        let tools = vec![DiscoveredTool {
            server: "s".to_string(),
            name: "search".to_string(),
            description: "web search".to_string(),
        }];
        let (sys, user) = schema_prompt(&tools);
        assert!(sys.contains("AgentPolicy"));
        assert!(user.contains("search"));
        assert!(user.contains("web search"));
    }

    #[test]
    fn policy_prompt_includes_agent_and_tools() {
        let agent = AgentSpec {
            name: "researcher".to_string(),
            description: "searches".to_string(),
            permitted_tools: vec!["search".to_string()],
        };
        let tools = vec![DiscoveredTool {
            server: "s".to_string(),
            name: "search".to_string(),
            description: "web search".to_string(),
        }];
        let (_sys, user) = policy_prompt(&agent, "clawdia", &tools);
        assert!(user.contains("researcher"));
        assert!(user.contains("clawdia"));
        assert!(user.contains("search"));
    }

    #[test]
    fn retry_prompt_includes_errors_and_prev() {
        let r = retry_prompt("PREV", "ERRS");
        assert!(r.contains("PREV"));
        assert!(r.contains("ERRS"));
    }
}
