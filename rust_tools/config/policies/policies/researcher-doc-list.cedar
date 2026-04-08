// @id("researcher-doc-list")
@description("Researcher can list available campaign documents")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);
