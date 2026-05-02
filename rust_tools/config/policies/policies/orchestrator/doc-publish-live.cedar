// @id("orchestrator-doc-publish-live")
@description("Orchestrator can publish a campaign document as live, gated by human approval")
@tool("doc_publish_live")
@gated_by("approval")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"doc_publish_live",
  resource == AgentPolicy::System::"clawdia"
)
when {
  context has approval &&
  context.approval.status == "approved" &&
  context.approval.action_kind == "doc.publish_live"
};
