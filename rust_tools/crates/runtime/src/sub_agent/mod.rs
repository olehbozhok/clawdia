pub mod cancel;
#[cfg(test)]
mod cascade_test;
pub mod fail;
pub mod inspect;
pub mod mcp_tools;
pub mod outcome;
pub mod registry;
pub mod render;
pub mod spawn;
pub mod state;

/// Tool names that every agent has access to unconditionally.
/// These are part of the runtime contract (spawn / inspect / cancel children,
/// terminate own session, request and inspect own approvals) — not privileges
/// to be granted per-agent. Cedar policies still gate the underlying gated
/// actions; `approval_*` only manages the lifecycle.
pub const BUILTIN_AGENT_TOOLS: &[&str] = &[
    "agent_spawn",
    "agent_get",
    "agent_cancel",
    "session_done",
    "session_fail",
    "approval_request",
    "approval_status",
    "approval_describe",
    "approval_execute",
    "approval_list_mine",
];
