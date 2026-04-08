// @id("copywriter-doc-write-content")
@description("Copywriter can write campaign content to a document")
@tool("doc_write_content")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"doc_write_content",
  resource == AgentPolicy::System::"clawdia"
);
