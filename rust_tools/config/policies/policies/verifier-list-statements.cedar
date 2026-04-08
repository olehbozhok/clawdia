// @id("verifier-list-statements")
@description("Verifier can list statements in a campaign document")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);
