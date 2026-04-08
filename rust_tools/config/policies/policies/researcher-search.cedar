// @id("researcher-search")
@description("Researcher can search the web for marine conservation sources")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);
