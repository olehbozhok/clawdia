// @id("copywriter-get-verified")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get_verified"
);
