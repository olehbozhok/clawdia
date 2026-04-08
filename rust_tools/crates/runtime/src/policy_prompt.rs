use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

/// A single permission entry extracted from a Cedar policy annotation.
#[derive(Debug, Clone)]
pub struct PermissionEntry {
    pub tool: String,
    pub description: String,
}

/// Parse all `.cedar` files in a policies directory, extract @tool and @description
/// annotations, and return entries grouped by agent name.
///
/// Agent name is derived from the filename prefix before the first `-`.
/// E.g., `researcher-search.cedar` → agent "researcher".
pub fn load_permissions_from_policies(
    policy_store_dir: &Path,
) -> Result<HashMap<String, Vec<PermissionEntry>>> {
    let mut agent_permissions: HashMap<String, Vec<PermissionEntry>> = HashMap::new();

    let policies_path = policy_store_dir.join("policies");
    if !policies_path.exists() {
        return Ok(agent_permissions);
    }

    for entry in std::fs::read_dir(&policies_path)
        .with_context(|| format!("reading {}", policies_path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("cedar") {
            continue;
        }

        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Agent name is prefix before first '-'
        let agent_name = filename
            .split('-')
            .next()
            .unwrap_or(filename)
            .to_string();

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        if let Some(entry) = parse_single_policy_annotations(&content) {
            agent_permissions
                .entry(agent_name)
                .or_default()
                .push(entry);
        }
    }

    Ok(agent_permissions)
}

/// Parse @tool and @description annotations from a single-policy Cedar file.
fn parse_single_policy_annotations(content: &str) -> Option<PermissionEntry> {
    let mut tool: Option<String> = None;
    let mut description: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = extract_annotation(trimmed, "tool") {
            tool = Some(val);
        } else if let Some(val) = extract_annotation(trimmed, "description") {
            description = Some(val);
        }
    }

    match (tool, description) {
        (Some(t), Some(d)) => Some(PermissionEntry { tool: t, description: d }),
        _ => None,
    }
}

fn extract_annotation(line: &str, name: &str) -> Option<String> {
    let prefix = format!("@{name}(\"");
    let stripped = line.strip_prefix(&prefix)?;
    let value = stripped.strip_suffix("\")")?;
    Some(value.to_string())
}

/// Build a permissions summary string to append to an agent's system prompt.
pub fn build_permissions_prompt(entries: &[PermissionEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut lines = vec!["\n## Your permissions\n".to_string()];
    lines.push("You are authorized to use the following tools:".to_string());
    for entry in entries {
        lines.push(format!("- {} — {}", entry.tool, entry.description));
    }
    lines.push("\nAny tool call outside this list will be denied.".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_policy_extracts_both_annotations() {
        let cedar = r#"
// @id("researcher-search")
@description("Researcher can search the web")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);
"#;
        let entry = parse_single_policy_annotations(cedar).unwrap();
        assert_eq!(entry.tool, "search");
        assert_eq!(entry.description, "Researcher can search the web");
    }

    #[test]
    fn parse_single_policy_returns_none_when_missing_description() {
        let cedar = r#"
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);
"#;
        assert!(parse_single_policy_annotations(cedar).is_none());
    }

    #[test]
    fn parse_single_policy_returns_none_when_missing_tool() {
        let cedar = r#"
@description("Can search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);
"#;
        assert!(parse_single_policy_annotations(cedar).is_none());
    }

    #[test]
    fn build_prompt_formats_correctly() {
        let entries = vec![
            PermissionEntry {
                tool: "search".to_string(),
                description: "Search the web".to_string(),
            },
            PermissionEntry {
                tool: "doc_list".to_string(),
                description: "List campaigns".to_string(),
            },
        ];
        let prompt = build_permissions_prompt(&entries);
        assert!(prompt.contains("## Your permissions"));
        assert!(prompt.contains("- search — Search the web"));
        assert!(prompt.contains("- doc_list — List campaigns"));
        assert!(prompt.contains("Any tool call outside this list will be denied."));
    }

    #[test]
    fn build_prompt_empty_entries_returns_empty() {
        assert!(build_permissions_prompt(&[]).is_empty());
    }

    // Integration test using real policy files
    #[test]
    fn load_permissions_from_real_policies() {
        let policy_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/policies");

        if !policy_dir.exists() {
            eprintln!("Skipping: policy dir not found");
            return;
        }

        let perms = load_permissions_from_policies(&policy_dir).unwrap();

        // researcher should have entries
        let researcher = perms.get("researcher").expect("researcher should have permissions");
        assert!(researcher.len() >= 5, "researcher should have at least 5 tools");

        // check a specific one exists
        assert!(researcher.iter().any(|e| e.tool == "search"), "researcher should have search tool");

        // copywriter should have entries
        let copywriter = perms.get("copywriter").expect("copywriter should have permissions");
        assert!(copywriter.iter().any(|e| e.tool == "doc_write_content"));

        // orchestrator should have entries
        assert!(perms.contains_key("orchestrator"));
        assert!(perms.contains_key("verifier"));
    }
}
