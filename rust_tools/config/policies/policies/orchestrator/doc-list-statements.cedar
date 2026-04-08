// @id("orchestrator-doc-list-statements")
@description("Orchestrator can list statements in a campaign document")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_list_statements",
  resource == AgentPolicy::System::"clawdia"
);
