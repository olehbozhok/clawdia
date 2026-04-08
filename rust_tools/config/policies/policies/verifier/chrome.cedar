// @id("verifier-chrome")
@description("Verifier can retrieve web content via Chrome browser")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"chrome_get_web_content",
  resource == AgentPolicy::System::"clawdia"
);
