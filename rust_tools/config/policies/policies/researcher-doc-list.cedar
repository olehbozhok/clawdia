// @id("researcher-doc-list")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
