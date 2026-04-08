// @id("orchestrator-doc-status")
@description("Orchestrator can check the status of a campaign document")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_status",
  resource == AgentPolicy::System::"clawdia"
);
