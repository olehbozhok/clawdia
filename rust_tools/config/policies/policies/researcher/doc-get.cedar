// @id("researcher-doc-get")
@description("Researcher can read a campaign document")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"doc_get",
  resource == AgentPolicy::System::"clawdia"
);
