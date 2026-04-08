// @id("orchestrator-publish-draft")
@description("Orchestrator can publish a campaign document as a draft")
@tool("doc_publish_draft")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_publish_draft"
);
