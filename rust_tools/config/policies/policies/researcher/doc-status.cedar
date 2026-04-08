// @id("researcher-doc-status")
@description("Researcher can check the status of a campaign document")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"doc_status",
  resource == AgentPolicy::System::"clawdia"
);
