// @id("researcher-search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);
