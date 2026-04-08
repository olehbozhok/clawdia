// @id("copywriter-doc-list")
@description("Copywriter can list available campaign documents")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
