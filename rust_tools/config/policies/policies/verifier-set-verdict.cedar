// @id("verifier-set-verdict")
@description("Verifier can set a verification verdict on a statement")
@tool("doc_set_verdict")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_set_verdict"
);
