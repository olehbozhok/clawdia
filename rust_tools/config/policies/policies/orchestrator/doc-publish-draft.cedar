// @id("orchestrator-doc-publish-draft")
@description("Orchestrator can publish a campaign document as a draft")
@tool("doc_publish_draft")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_publish_draft",
  resource == AgentPolicy::System::"clawdia"
);
