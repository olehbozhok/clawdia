@description("Orchestrator can create new campaigns")
@tool("doc_create")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_create"
);

@description("Orchestrator can assemble campaign content")
@tool("doc_assemble")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_assemble"
);

@description("Orchestrator can publish campaign draft")
@tool("doc_publish_draft")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_publish_draft"
);

@description("Orchestrator can publish campaign live")
@tool("doc_publish_live")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_manage",
  resource == AgentPolicy::Tool::"doc_publish_live"
);

@description("Orchestrator can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Orchestrator can list statements")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);

@description("Orchestrator can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);

@description("Orchestrator can read campaign data")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
