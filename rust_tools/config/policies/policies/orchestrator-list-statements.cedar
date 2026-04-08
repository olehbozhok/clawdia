// @id("orchestrator-list-statements")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);
