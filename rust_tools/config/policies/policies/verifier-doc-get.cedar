// @id("verifier-doc-get")
@description("Verifier can read a campaign document")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
