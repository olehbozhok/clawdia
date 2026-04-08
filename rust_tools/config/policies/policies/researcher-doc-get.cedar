// @id("researcher-doc-get")
@description("Researcher can read a campaign document")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
