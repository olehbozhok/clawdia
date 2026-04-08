// @id("researcher-list-statements")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);
