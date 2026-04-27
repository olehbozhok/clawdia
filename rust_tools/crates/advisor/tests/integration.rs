use advisor::{AdvisorInput, AgentSpec, DiscoveredTool, FakeLlmClient};

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

const VALID_POLICY_RESEARCHER: &str = r#"
@description("researcher search")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"search",
  resource == AgentPolicy::System::"clawdia"
);
"#;

#[tokio::test]
async fn end_to_end_generates_policy_store_with_fake_llm() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("policies");

    let input = AdvisorInput {
        agents: vec![AgentSpec {
            name: "researcher".into(),
            description: "searches the web".into(),
            permitted_tools: vec!["search".into()],
        }],
        tools: vec![DiscoveredTool {
            server: "search-server".into(),
            name: "search".into(),
            description: "web search".into(),
        }],
        policy_store_id: "deadbeef01234567".into(),
        system_entity_id: "clawdia".into(),
        domain_hint: None,
    };

    let client = FakeLlmClient::new(vec![VALID_SCHEMA, VALID_POLICY_RESEARCHER]);

    let out = advisor::run(&client, input, out_dir.clone()).await.unwrap();

    assert!(out.output_dir.join("schema.cedarschema").exists());
    assert!(out.output_dir.join("policies/researcher.cedar").exists());
    assert!(out.output_dir.join("entities/agents.json").exists());
    assert!(out.output_dir.join("entities/system.json").exists());
    assert!(out.output_dir.join("metadata.json").exists());

    let schema = std::fs::read_to_string(out.output_dir.join("schema.cedarschema")).unwrap();
    assert!(schema.contains("AgentPolicy"));

    let agents = std::fs::read_to_string(out.output_dir.join("entities/agents.json")).unwrap();
    assert!(agents.contains("researcher"));

    let system = std::fs::read_to_string(out.output_dir.join("entities/system.json")).unwrap();
    assert!(system.contains("clawdia"));
}

#[tokio::test]
async fn generated_store_loads_into_cedarling() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("policies");

    let input = AdvisorInput {
        agents: vec![AgentSpec {
            name: "researcher".into(),
            description: "searches".into(),
            permitted_tools: vec!["search".into()],
        }],
        tools: vec![DiscoveredTool {
            server: "s".into(),
            name: "search".into(),
            description: "web search".into(),
        }],
        policy_store_id: "deadbeef01234567".into(),
        system_entity_id: "clawdia".into(),
        domain_hint: None,
    };

    let client = FakeLlmClient::new(vec![VALID_SCHEMA, VALID_POLICY_RESEARCHER]);
    let out = advisor::run(&client, input, out_dir.clone()).await.unwrap();

    let authz = runtime::cedar_authz::CedarAuthz::from_directory(&out.output_dir)
        .await
        .expect("Cedarling should load advisor-generated policies");

    let result = authz
        .authorize("researcher", "search", &serde_json::json!({}))
        .await;
    assert_eq!(
        result.decision,
        tools::authz::AuthorizationDecision::Allow,
        "researcher should be allowed to search"
    );
}
