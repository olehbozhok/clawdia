// @id("copywriter-write-content")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_write_content"
);
