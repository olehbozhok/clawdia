// @id("orchestrator-publish-live")
@description("Orchestrator can publish a campaign document as live")
@tool("doc_publish_live")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_publish_live"
);
