// @id("orchestrator-doc-list")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
