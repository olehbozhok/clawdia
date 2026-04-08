// @id("researcher-doc-get")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
