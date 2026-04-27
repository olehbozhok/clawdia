//! Advisor — generates a Cedarling policy store (schema + policies + entities)
//! from a deployment's MCP tool catalog and agent role list using an LLM.

pub mod discovery;
pub mod generator;
pub mod llm;
pub mod prompts;
pub mod types;
pub mod validator;
pub mod writer;

pub use types::{AdvisorInput, AdvisorOutput, AgentSpec, Artifacts, DiscoveredTool};
