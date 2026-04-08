// @id("verifier-doc-list-statements")
@description("Verifier can list statements in a campaign document")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"doc_list_statements",
  resource == AgentPolicy::System::"clawdia"
);
