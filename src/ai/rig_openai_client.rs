use anyhow::{Context, Result};
use rig::{
    client::{CompletionClient, VerifyClient},
    completion::TypedPrompt,
    providers::openai,
};
use std::{future::IntoFuture, time::Instant};
use tokio::time::{self, Duration, MissedTickBehavior};
use tracing::info;

use crate::config::OpenAiConfig;

use super::{
    classify::{AiDecision, AiInput},
    prompts::{build_prompt, SYSTEM_PREAMBLE},
};

#[derive(Debug, Clone)]
pub struct RigOpenAiClient {
    base_url: String,
    model: String,
    api_key: Option<String>,
    api_key_env: Option<String>,
}

impl RigOpenAiClient {
    pub fn new(config: &OpenAiConfig) -> Self {
        Self {
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            api_key: config.api_key.clone(),
            api_key_env: config.api_key_env.clone(),
        }
    }

    #[tracing::instrument(name = "openai_classify", skip_all, fields(model = %self.model))]
    pub async fn classify(&self, input: &AiInput) -> Result<AiDecision> {
        let prompt = build_prompt(input);
        info!(
            event = "ai_classify_start",
            model = %self.model,
            dom_len = input.dom_element.len()
        );

        let client = self.build_client()?;

        let agent = client.agent(&self.model).preamble(SYSTEM_PREAMBLE).build();

        match classify_once(&agent, &prompt, &self.model, 1).await {
            Ok(v) => Ok(v),
            Err(_first_err) => {
                info!(
                    event = "ai_classify_retry",
                    reason = "invalid_schema_or_json"
                );
                classify_once(&agent, &prompt, &self.model, 2)
                    .await
                    .context("Rig/OpenAI classify retry failed")
            }
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }

    pub async fn healthcheck(&self) -> Result<()> {
        let client = self.build_client()?;
        client
            .verify()
            .await
            .map_err(|_| anyhow::anyhow!("OpenAI verify failed"))?;

        Ok(())
    }

    pub async fn unload_model(&self) -> Result<()> {
        // OpenAI is stateless per request; there is no model unload endpoint.
        info!(event = "openai_unload_not_required", model = %self.model);
        Ok(())
    }

    fn build_client(&self) -> Result<openai::Client> {
        let api_key = self.resolve_api_key()?;

        openai::Client::builder()
            .api_key(&api_key)
            .base_url(&self.base_url)
            .build()
            .context("failed to build Rig OpenAI client")
    }

    fn resolve_api_key(&self) -> Result<String> {
        if let Some(raw) = &self.api_key {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }

        if let Some(raw) = &self.api_key_env {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                // Backward compatibility for misconfigured files that put raw key in api_key_env.
                if looks_like_openai_key(trimmed) {
                    return Ok(trimmed.to_string());
                }
                return std::env::var(trimmed)
                    .map_err(|_| anyhow::anyhow!("OpenAI API key env variable not set"));
            }
        }

        Err(anyhow::anyhow!("OpenAI API key not configured"))
    }
}

fn looks_like_openai_key(value: &str) -> bool {
    value.starts_with("sk-")
}

async fn classify_once(
    agent: &(impl TypedPrompt + ?Sized),
    prompt: &str,
    model: &str,
    attempt: u8,
) -> Result<AiDecision> {
    let response = prompt_typed_with_heartbeat(agent, prompt, model, attempt).await?;
    Ok(response.sanitized())
}

#[tracing::instrument(name = "openai_prompt_with_heartbeat", skip_all, fields(model = %model, attempt))]
async fn prompt_typed_with_heartbeat(
    agent: &(impl TypedPrompt + ?Sized),
    prompt: &str,
    model: &str,
    attempt: u8,
) -> Result<AiDecision> {
    let started = Instant::now();
    let mut fut = Box::pin(agent.prompt_typed::<AiDecision>(prompt).into_future());

    let mut heartbeat = time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;

    loop {
        tokio::select! {
            result = &mut fut => {
                let response = result.context("Rig/OpenAI typed prompt failed")?;
                info!(
                    event = "ai_classify_completed",
                    model = %model,
                    attempt,
                    elapsed_ms = started.elapsed().as_millis()
                );
                return Ok(response);
            }
            _ = heartbeat.tick() => {
                info!(
                    event = "ai_classify_heartbeat",
                    model = %model,
                    attempt,
                    elapsed_s = started.elapsed().as_secs()
                );
            }
        }
    }
}
