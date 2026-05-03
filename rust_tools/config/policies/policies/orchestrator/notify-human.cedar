// @id("orchestrator-notify-human")
@description("Orchestrator may notify the human (design §17). Sub-agents are denied by default — they must report failures upstream via session_fail, not directly to the human.")
@tool("notify_human")
permit(
  principal == AgentPolicy::Agent::"orchestrator",
  action == AgentPolicy::Action::"notify_human",
  resource == AgentPolicy::System::"clawdia"
);
