use anyhow::Result;
use async_trait::async_trait;

use crate::{
    ai::rig_openai_client::RigOpenAiClient,
    config::OpenAiConfig,
    domain::ai::{AiDecision, AiInput},
    ports::ai::AiClassifier,
};

/// OpenAI-backed `AiClassifier` adapter.
pub struct OpenAiClassifier {
    inner: RigOpenAiClient,
}

impl OpenAiClassifier {
    /// Builds an adapter from app config.
    pub fn new(config: &OpenAiConfig) -> Self {
        Self {
            inner: RigOpenAiClient::new(config),
        }
    }
}

#[async_trait]
impl AiClassifier for OpenAiClassifier {
    async fn classify(&self, input: &AiInput) -> Result<AiDecision> {
        self.inner.classify(input).await
    }

    async fn healthcheck(&self) -> Result<()> {
        self.inner.healthcheck().await
    }

    async fn unload_model(&self) -> Result<()> {
        self.inner.unload_model().await
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}
