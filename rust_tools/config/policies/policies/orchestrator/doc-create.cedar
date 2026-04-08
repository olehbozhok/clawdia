// @id("orchestrator-doc-create")
@description("Orchestrator can create new campaign documents")
@tool("doc_create")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_create",
  resource == AgentPolicy::System::"clawdia"
);
