@description("Verifier can fetch page content to check sources")
@tool("fetch_content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
);

@description("Verifier can use browser to check sources")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);

@description("Verifier can set verdict on statements")
@tool("doc_set_verdict")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_set_verdict"
);

@description("Verifier can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Verifier can list statements for verification")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);

@description("Verifier can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);

@description("Verifier can read campaign data")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"verifier",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
