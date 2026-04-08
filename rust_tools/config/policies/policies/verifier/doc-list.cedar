// @id("verifier-doc-list")
@description("Verifier can list available campaign documents")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"doc_list",
  resource == AgentPolicy::System::"clawdia"
);
