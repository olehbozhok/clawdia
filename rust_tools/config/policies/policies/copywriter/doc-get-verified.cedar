// @id("copywriter-doc-get-verified")
@description("Copywriter can retrieve verified statements for a campaign")
@tool("doc_get_verified")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"doc_get_verified",
  resource == AgentPolicy::System::"clawdia"
);
