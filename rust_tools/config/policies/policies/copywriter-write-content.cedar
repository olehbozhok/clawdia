// @id("copywriter-write-content")
@description("Copywriter can write campaign content to a document")
@tool("doc_write_content")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_write_content"
);
