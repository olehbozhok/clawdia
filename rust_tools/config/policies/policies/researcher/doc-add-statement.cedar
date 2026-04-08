// @id("researcher-doc-add-statement")
@description("Researcher can add sourced statements to a campaign document")
@tool("doc_add_statement")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"doc_add_statement",
  resource == AgentPolicy::System::"clawdia"
);
