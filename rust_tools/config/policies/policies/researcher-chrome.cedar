// @id("researcher-chrome")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);
