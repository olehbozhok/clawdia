// @id("copywriter-doc-list")
@description("Copywriter can list available campaign documents")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"doc_list",
  resource == AgentPolicy::System::"clawdia"
);
