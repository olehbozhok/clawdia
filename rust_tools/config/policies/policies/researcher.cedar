@description("Researcher can search the web for marine conservation sources")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);

@description("Researcher can fetch page content")
@tool("fetch_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
);

@description("Researcher can use browser to bypass bot detection")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);

@description("Researcher can add statements to campaigns")
@tool("doc_add_statement")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_add_statement"
);

@description("Researcher can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Researcher can list statements to avoid duplicates")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);

@description("Researcher can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);

@description("Researcher can read campaign data")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
