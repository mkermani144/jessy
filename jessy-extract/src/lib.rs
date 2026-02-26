use std::future::Future;

use anyhow::{anyhow, Result};
use jessy_core::JobStage;
use sha2::{Digest, Sha256};

pub const STEP_STAGE: JobStage = JobStage::Extract;

#[derive(Debug, Clone)]
pub struct ExtractSeed {
    pub platform: String,
    pub canonical_url: String,
    pub source_ref: String,
    pub source_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExtractRunInput {
    pub seeds: Vec<ExtractSeed>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ExtractRunOutput {
    pub processed: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractPreparedRecord {
    pub dedupe_key: String,
    pub platform: String,
    pub canonical_url: String,
    pub source_ref: String,
    pub source_cursor: Option<String>,
    pub legacy_source_page_index: i64,
    pub current_stage: JobStage,
    pub status_meta: String,
}

pub trait ExtractRepo: Send + Sync {
    fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send;
    fn upsert_extracted<'a>(
        &'a self,
        record: &'a ExtractPreparedRecord,
    ) -> impl Future<Output = Result<()>> + Send + 'a;
}

pub struct ExtractService<R: ExtractRepo> {
    repo: R,
}

impl<R: ExtractRepo> ExtractService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub async fn run(&self, input: ExtractRunInput) -> Result<ExtractRunOutput> {
        if input.seeds.is_empty() {
            return Ok(ExtractRunOutput { processed: 0 });
        }

        let reason = normalized_reason(&input.reason);
        self.repo.ensure_ready().await?;

        for seed in &input.seeds {
            let prepared = prepare_record(seed, &reason)?;
            self.repo.upsert_extracted(&prepared).await?;
        }

        Ok(ExtractRunOutput {
            processed: input.seeds.len(),
        })
    }
}

pub const fn step_name() -> &'static str {
    STEP_STAGE.as_str()
}

pub fn prepare_record(seed: &ExtractSeed, reason: &str) -> Result<ExtractPreparedRecord> {
    let platform = seed.platform.trim().to_ascii_lowercase();
    if platform.is_empty() {
        return Err(anyhow!("extract seed platform is empty"));
    }

    let canonical_url = seed.canonical_url.trim();
    if canonical_url.is_empty() {
        return Err(anyhow!("extract seed canonical_url is empty"));
    }

    let source_ref = if seed.source_ref.trim().is_empty() {
        canonical_url.to_string()
    } else {
        seed.source_ref.trim().to_string()
    };

    let source_cursor = seed
        .source_cursor
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);

    Ok(ExtractPreparedRecord {
        dedupe_key: dedupe_key(&platform, canonical_url),
        platform: platform.clone(),
        canonical_url: canonical_url.to_string(),
        source_ref,
        legacy_source_page_index: legacy_source_page_index(source_cursor.as_deref()),
        source_cursor,
        current_stage: STEP_STAGE,
        status_meta: format!("{}:{}:{}", step_name(), platform, normalized_reason(reason)),
    })
}

fn normalized_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        "manual_seed".to_string()
    } else {
        trimmed.to_string()
    }
}

fn dedupe_key(platform: &str, canonical_url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(platform.as_bytes());
    hasher.update(b":");
    hasher.update(canonical_url.as_bytes());
    hex::encode(hasher.finalize())
}

fn legacy_source_page_index(source_cursor: Option<&str>) -> i64 {
    match source_cursor {
        Some(cursor) => cursor.parse::<i64>().ok().filter(|v| *v > 0).unwrap_or(1),
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_name_is_extract() {
        assert_eq!(step_name(), "extract");
    }

    #[test]
    fn prepare_record_applies_policy() {
        let seed = ExtractSeed {
            platform: "Telegram".to_string(),
            canonical_url: "https://t.me/channel/123".to_string(),
            source_ref: "https://t.me/channel".to_string(),
            source_cursor: Some("msg:123".to_string()),
        };

        let record = prepare_record(&seed, "smoke").expect("record");
        assert_eq!(record.platform, "telegram");
        assert_eq!(record.current_stage, JobStage::Extract);
        assert_eq!(record.status_meta, "extract:telegram:smoke");
        assert_eq!(record.legacy_source_page_index, 1);
    }

    #[test]
    fn prepare_record_rejects_empty_platform() {
        let seed = ExtractSeed {
            platform: " ".to_string(),
            canonical_url: "https://example.com/jobs/1".to_string(),
            source_ref: "".to_string(),
            source_cursor: None,
        };
        assert!(prepare_record(&seed, "x").is_err());
    }

    #[test]
    fn prepare_record_uses_numeric_cursor_for_legacy_index() {
        let seed = ExtractSeed {
            platform: "linkedin".to_string(),
            canonical_url: "https://example.com/jobs/1".to_string(),
            source_ref: "https://example.com/search".to_string(),
            source_cursor: Some("7".to_string()),
        };
        let record = prepare_record(&seed, "x").expect("record");
        assert_eq!(record.legacy_source_page_index, 7);
    }
}
