@description("Copywriter can write campaign content")
@tool("doc_write_content")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_write_content"
);

@description("Copywriter can read verified campaign data only")
@tool("doc_get_verified")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get_verified"
);

@description("Copywriter can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Copywriter can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"copywriter",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);
