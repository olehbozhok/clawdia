// @id("orchestrator-doc-list")
@description("Orchestrator can list available campaign documents")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
