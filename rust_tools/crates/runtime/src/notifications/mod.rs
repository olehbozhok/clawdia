//! Human notification primitive (design §17). Fire-and-forget signals from
//! agents to humans. Distinct from approvals: no Wait, no inbox, no session
//! state change.
//!
//! `Severity` lives here as the canonical home (D10) and is imported by Plan
//! 06 for TUI rendering. Do not redefine elsewhere.

pub mod emit;
pub mod types;
