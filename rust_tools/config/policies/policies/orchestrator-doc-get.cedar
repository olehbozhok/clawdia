// @id("orchestrator-doc-get")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
