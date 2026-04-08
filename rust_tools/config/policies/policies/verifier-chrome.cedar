// @id("verifier-chrome")
@description("Verifier can retrieve web content via Chrome browser")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);
