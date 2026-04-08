// @id("orchestrator-doc-list")
@description("Orchestrator can list available campaign documents")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_list",
  resource == AgentPolicy::System::"clawdia"
);
