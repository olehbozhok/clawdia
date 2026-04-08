// @id("verifier-chrome")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);
