// @id("verifier-set-verdict")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_set_verdict"
);
