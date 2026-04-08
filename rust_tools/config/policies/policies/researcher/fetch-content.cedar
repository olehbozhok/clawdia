// @id("researcher-fetch-content")
@description("Researcher can fetch web page content by URL")
@tool("fetch_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"fetch_content",
  resource == AgentPolicy::System::"clawdia"
);
