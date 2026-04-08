// @id("copywriter-doc-list")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
