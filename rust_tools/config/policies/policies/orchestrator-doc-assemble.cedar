// @id("orchestrator-doc-assemble")
@description("Orchestrator can assemble a campaign document from verified statements")
@tool("doc_assemble")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_assemble"
);
