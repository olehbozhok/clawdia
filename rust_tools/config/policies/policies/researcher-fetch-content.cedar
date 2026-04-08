// @id("researcher-fetch-content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
);
