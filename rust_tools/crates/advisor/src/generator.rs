//! Drives the LLM through schema and policy generation with a bounded retry
//! loop on validation failures.

use anyhow::{Context, Result};
use cedar_policy::Schema;
use std::collections::BTreeMap;

use crate::llm::LlmClient;
use crate::prompts::{policy_prompt, retry_prompt, schema_prompt};
use crate::types::{AgentSpec, DiscoveredTool};
use crate::validator::{validate_policies, validate_schema};

const MAX_RETRIES: usize = 3;

/// Generate a Cedar schema, retrying on validation failure.
pub async fn generate_schema(
    client: &dyn LlmClient,
    tools: &[DiscoveredTool],
) -> Result<(String, Schema)> {
    let (sys, user) = schema_prompt(tools);
    let mut last_output = client.complete(&sys, &user).await?;

    for attempt in 0..MAX_RETRIES {
        match validate_schema(&last_output) {
            Ok(schema) => return Ok((last_output, schema)),
            Err(e) => {
                if attempt + 1 == MAX_RETRIES {
                    anyhow::bail!("schema validation exhausted after {MAX_RETRIES} attempts: {e}");
                }
                let user = retry_prompt(&last_output, &e.to_string());
                last_output = client.complete(&sys, &user).await?;
            }
        }
    }
    unreachable!()
}

/// Generate one policy file per agent, retrying each on validation failure.
pub async fn generate_policies(
    client: &dyn LlmClient,
    schema: &Schema,
    agents: &[AgentSpec],
    system_entity_id: &str,
    tools: &[DiscoveredTool],
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for agent in agents {
        let (sys, user) = policy_prompt(agent, system_entity_id, tools);
        let mut last_output = client.complete(&sys, &user).await?;

        let mut ok = false;
        for attempt in 0..MAX_RETRIES {
            match validate_policies(schema, &last_output) {
                Ok(()) => {
                    ok = true;
                    break;
                }
                Err(e) => {
                    if attempt + 1 == MAX_RETRIES {
                        return Err(e).with_context(|| {
                            format!(
                                "policy validation exhausted for agent `{}` after {MAX_RETRIES} attempts",
                                agent.name
                            )
                        });
                    }
                    let user = retry_prompt(&last_output, &e.to_string());
                    last_output = client.complete(&sys, &user).await?;
                }
            }
        }
        if ok {
            out.insert(agent.name.clone(), last_output);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::FakeLlmClient;

    const VALID_SCHEMA: &str = r#"
namespace AgentPolicy {
  entity Agent = { agent_type: String };
  entity System;
  action "search" appliesTo {
    principal: [Agent],
    resource: [System],
    context: {}
  };
}
"#;

    const VALID_POLICY: &str = r#"
@description("d")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"search",
  resource == AgentPolicy::System::"clawdia"
);
"#;

    fn one_tool() -> Vec<DiscoveredTool> {
        vec![DiscoveredTool {
            server: "s".into(),
            name: "search".into(),
            description: "web search".into(),
        }]
    }

    fn one_agent() -> Vec<AgentSpec> {
        vec![AgentSpec {
            name: "researcher".into(),
            description: "searches".into(),
            permitted_tools: vec!["search".into()],
        }]
    }

    #[tokio::test]
    async fn schema_generated_on_first_try() {
        let client = FakeLlmClient::new(vec![VALID_SCHEMA]);
        let (src, _schema) = generate_schema(&client, &one_tool()).await.unwrap();
        assert!(src.contains("namespace AgentPolicy"));
    }

    #[tokio::test]
    async fn schema_recovers_on_retry() {
        let client = FakeLlmClient::new(vec!["broken garbage", VALID_SCHEMA]);
        let (src, _) = generate_schema(&client, &one_tool()).await.unwrap();
        assert!(src.contains("AgentPolicy"));
        assert_eq!(client.calls().len(), 2);
    }

    #[tokio::test]
    async fn schema_fails_after_max_retries() {
        let client = FakeLlmClient::new(vec!["bad", "still bad", "still bad again"]);
        let err = generate_schema(&client, &one_tool()).await.unwrap_err();
        assert!(err.to_string().contains("exhausted"));
    }

    #[tokio::test]
    async fn policies_generated_per_agent() {
        let schema_client = FakeLlmClient::new(vec![VALID_SCHEMA]);
        let (_, schema) = generate_schema(&schema_client, &one_tool()).await.unwrap();

        let policy_client = FakeLlmClient::new(vec![VALID_POLICY]);
        let policies = generate_policies(&policy_client, &schema, &one_agent(), "clawdia", &one_tool())
            .await
            .unwrap();
        assert_eq!(policies.len(), 1);
        assert!(policies["researcher"].contains("researcher"));
    }
}
