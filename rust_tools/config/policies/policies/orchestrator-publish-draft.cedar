// @id("orchestrator-publish-draft")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_publish_draft"
);
