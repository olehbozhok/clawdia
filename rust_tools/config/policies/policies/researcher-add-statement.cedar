// @id("researcher-add-statement")
@description("Researcher can add sourced statements to a campaign document")
@tool("doc_add_statement")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_add_statement"
);
