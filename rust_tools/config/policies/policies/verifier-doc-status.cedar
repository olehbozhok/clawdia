// @id("verifier-doc-status")
@description("Verifier can check the status of a campaign document")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);
