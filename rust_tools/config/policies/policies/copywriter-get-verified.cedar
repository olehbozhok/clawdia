// @id("copywriter-get-verified")
@description("Copywriter can retrieve verified statements for a campaign")
@tool("doc_get_verified")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get_verified"
);
