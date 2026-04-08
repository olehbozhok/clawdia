// @id("orchestrator-doc-publish-live")
@description("Orchestrator can publish a campaign document as live")
@tool("doc_publish_live")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_publish_live",
  resource == AgentPolicy::System::"clawdia"
);
