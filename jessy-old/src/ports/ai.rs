use anyhow::Result;
use async_trait::async_trait;

use crate::domain::ai::{AiDecision, AiInput};

/// Port for AI extraction and runtime lifecycle operations.
#[async_trait]
pub trait AiClassifier: Send + Sync {
    /// Extracts one job independently (no shared conversation state).
    async fn classify(&self, input: &AiInput) -> Result<AiDecision>;
    /// Checks provider availability.
    async fn healthcheck(&self) -> Result<()>;
    /// Requests model unload to free RAM after a run.
    async fn unload_model(&self) -> Result<()>;
    /// Returns configured model identifier for logs/telemetry.
    fn model_name(&self) -> &str;
}
