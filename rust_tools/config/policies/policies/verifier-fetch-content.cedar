// @id("verifier-fetch-content")
@description("Verifier can fetch web page content to cross-check sources")
@tool("fetch_content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
);
