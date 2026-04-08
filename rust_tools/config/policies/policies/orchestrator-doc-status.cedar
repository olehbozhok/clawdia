// @id("orchestrator-doc-status")
@description("Orchestrator can check the status of a campaign document")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);
