// @id("orchestrator-doc-get")
@description("Orchestrator can read a campaign document")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_get",
  resource == AgentPolicy::System::"clawdia"
);
