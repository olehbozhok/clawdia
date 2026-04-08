// @id("verifier-fetch-content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
);
