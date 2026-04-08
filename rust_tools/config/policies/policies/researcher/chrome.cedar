// @id("researcher-chrome")
@description("Researcher can retrieve web content via Chrome browser")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"chrome_get_web_content",
  resource == AgentPolicy::System::"clawdia"
);
