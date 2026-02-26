use std::future::Future;

use anyhow::{anyhow, Result};
use jessy_core::JobStage;
use sha2::{Digest, Sha256};

pub const STEP_STAGE: JobStage = JobStage::Load;

#[derive(Debug, Clone)]
pub struct LoadSeed {
    pub platform: String,
    pub canonical_url: String,
    pub source_ref: String,
    pub source_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoadRunInput {
    pub seeds: Vec<LoadSeed>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct LoadRunOutput {
    pub processed: usize,
}

#[derive(Debug, Clone)]
pub struct LoadPreparedRecord {
    pub dedupe_key: String,
    pub platform: String,
    pub canonical_url: String,
    pub source_ref: String,
    pub source_cursor: Option<String>,
    pub legacy_source_page_index: i64,
    pub current_stage: JobStage,
    pub status_meta: String,
}

pub trait LoadRepo: Send + Sync {
    fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send;
    fn upsert_loaded<'a>(
        &'a self,
        record: &'a LoadPreparedRecord,
    ) -> impl Future<Output = Result<()>> + Send + 'a;
}

pub struct LoadService<R: LoadRepo> {
    repo: R,
}

impl<R: LoadRepo> LoadService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub async fn run(&self, input: LoadRunInput) -> Result<LoadRunOutput> {
        if input.seeds.is_empty() {
            return Ok(LoadRunOutput { processed: 0 });
        }

        let reason = normalized_reason(&input.reason);
        self.repo.ensure_ready().await?;
        for seed in &input.seeds {
            let record = prepare_record(seed, &reason)?;
            self.repo.upsert_loaded(&record).await?;
        }

        Ok(LoadRunOutput {
            processed: input.seeds.len(),
        })
    }
}

pub const fn step_name() -> &'static str {
    STEP_STAGE.as_str()
}

pub fn prepare_record(seed: &LoadSeed, reason: &str) -> Result<LoadPreparedRecord> {
    let platform = seed.platform.trim().to_ascii_lowercase();
    if platform.is_empty() {
        return Err(anyhow!("load seed platform is empty"));
    }

    let canonical_url = seed.canonical_url.trim();
    if canonical_url.is_empty() {
        return Err(anyhow!("load seed canonical_url is empty"));
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

    Ok(LoadPreparedRecord {
        dedupe_key: dedupe_key(&platform, canonical_url),
        platform: platform.clone(),
        canonical_url: canonical_url.to_string(),
        source_ref,
        source_cursor: source_cursor.clone(),
        legacy_source_page_index: legacy_source_page_index(source_cursor.as_deref()),
        current_stage: STEP_STAGE,
        status_meta: format!("{}:{}:{}", step_name(), platform, normalized_reason(reason)),
    })
}

fn normalized_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        "manual_load".to_string()
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
    fn step_name_is_load() {
        assert_eq!(step_name(), "load");
    }

    #[test]
    fn prepare_record_applies_load_policy() {
        let seed = LoadSeed {
            platform: "LinkedIn".to_string(),
            canonical_url: "https://www.linkedin.com/jobs/view/42".to_string(),
            source_ref: "https://www.linkedin.com/jobs/search/".to_string(),
            source_cursor: Some("3".to_string()),
        };
        let record = prepare_record(&seed, "smoke").expect("record");
        assert_eq!(record.current_stage, JobStage::Load);
        assert_eq!(record.status_meta, "load:linkedin:smoke");
        assert_eq!(record.legacy_source_page_index, 3);
    }

    #[test]
    fn prepare_record_rejects_empty_platform() {
        let seed = LoadSeed {
            platform: "".to_string(),
            canonical_url: "https://example.com/jobs/1".to_string(),
            source_ref: "".to_string(),
            source_cursor: None,
        };
        assert!(prepare_record(&seed, "x").is_err());
    }
}
