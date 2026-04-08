// @id("verifier-doc-list")
@description("Verifier can list available campaign documents")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
