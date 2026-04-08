// @id("orchestrator-doc-create")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_create"
);
