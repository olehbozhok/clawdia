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
/// terminate own session) — not privileges to be granted per-agent.
pub const BUILTIN_AGENT_TOOLS: &[&str] = &[
    "agent_spawn",
    "agent_get",
    "agent_cancel",
    "session_done",
    "session_fail",
];
