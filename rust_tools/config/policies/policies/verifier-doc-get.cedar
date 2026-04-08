// @id("verifier-doc-get")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
