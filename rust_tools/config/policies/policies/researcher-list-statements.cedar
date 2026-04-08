// @id("researcher-list-statements")
@description("Researcher can list statements in a campaign document")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);
