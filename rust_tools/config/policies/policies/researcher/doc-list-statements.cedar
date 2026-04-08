// @id("researcher-doc-list-statements")
@description("Researcher can list statements in a campaign document")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"doc_list_statements",
  resource == AgentPolicy::System::"clawdia"
);
