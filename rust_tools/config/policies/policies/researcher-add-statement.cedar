// @id("researcher-add-statement")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_add_statement"
);
