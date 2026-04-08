// @id("copywriter-doc-status")
@description("Copywriter can check the status of a campaign document")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"doc_status",
  resource == AgentPolicy::System::"clawdia"
);
