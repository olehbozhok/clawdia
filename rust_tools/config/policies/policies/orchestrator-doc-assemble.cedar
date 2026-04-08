// @id("orchestrator-doc-assemble")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_assemble"
);
