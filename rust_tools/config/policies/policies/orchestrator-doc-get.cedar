// @id("orchestrator-doc-get")
@description("Orchestrator can read a campaign document")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
