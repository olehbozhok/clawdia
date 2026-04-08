// @id("copywriter-doc-status")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);
