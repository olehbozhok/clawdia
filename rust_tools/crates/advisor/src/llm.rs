//! LLM client abstraction so generation logic is testable without a live LLM.

use std::sync::{Arc, Mutex};

use anyhow::Result;

/// Async trait for completing a single prompt.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> Result<String>;
}

/// Test double: returns canned responses in order, one per call.
pub struct FakeLlmClient {
    responses: Arc<Mutex<Vec<String>>>,
    calls: Arc<Mutex<Vec<(String, String)>>>,
}

impl FakeLlmClient {
    pub fn new(responses: Vec<&str>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(
                responses.into_iter().map(|s| s.to_string()).collect(),
            )),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl LlmClient for FakeLlmClient {
    async fn complete(&self, system: &str, user: &str) -> Result<String> {
        self.calls
            .lock()
            .unwrap()
            .push((system.to_string(), user.to_string()));
        let mut q = self.responses.lock().unwrap();
        if q.is_empty() {
            anyhow::bail!("FakeLlmClient: no more canned responses");
        }
        Ok(q.remove(0))
    }
}

use rig::client::CompletionClient;
use rig::completion::Prompt;

pub struct RigClient {
    agent: rig::providers::deepseek::Client,
    model: String,
}

impl RigClient {
    pub fn new(api_key: &str, model: impl Into<String>) -> Result<Self> {
        let agent = rig::providers::deepseek::Client::new(api_key)?;
        Ok(Self {
            agent,
            model: model.into(),
        })
    }
}

#[async_trait::async_trait]
impl LlmClient for RigClient {
    async fn complete(&self, system: &str, user: &str) -> Result<String> {
        let agent = self
            .agent
            .agent(&self.model)
            .preamble(system)
            .build();
        let response = agent.prompt(user).await?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_returns_responses_in_order() {
        let c = FakeLlmClient::new(vec!["one", "two"]);
        assert_eq!(c.complete("s", "u1").await.unwrap(), "one");
        assert_eq!(c.complete("s", "u2").await.unwrap(), "two");
        assert!(c.complete("s", "u3").await.is_err());
    }

    #[tokio::test]
    async fn fake_records_calls() {
        let c = FakeLlmClient::new(vec!["x"]);
        c.complete("sys", "user").await.unwrap();
        assert_eq!(c.calls(), vec![("sys".to_string(), "user".to_string())]);
    }
}
