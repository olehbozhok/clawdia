// @id("researcher-chrome")
@description("Researcher can retrieve web content via Chrome browser")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);
