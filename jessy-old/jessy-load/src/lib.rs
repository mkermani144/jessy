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
    pub platform_filter: Option<String>,
    pub pending_limit: usize,
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

#[derive(Debug, Clone)]
pub struct LoadPendingSelection {
    pub platform_filter: Option<String>,
    pub limit: usize,
}

pub trait LoadRepo: Send + Sync {
    fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send;
    fn list_pending_extract_seeds<'a>(
        &'a self,
        selection: &'a LoadPendingSelection,
    ) -> impl Future<Output = Result<Vec<LoadSeed>>> + Send + 'a;
    fn upsert_loaded<'a>(
        &'a self,
        record: &'a LoadPreparedRecord,
    ) -> impl Future<Output = Result<()>> + Send + 'a;
    fn mark_extract_seed_loaded<'a>(
        &'a self,
        dedupe_key: &'a str,
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
        let reason = normalized_reason(&input.reason);
        let pending_selection =
            normalized_pending_selection(input.platform_filter, input.pending_limit)?;
        self.repo.ensure_ready().await?;
        let seeds = if input.seeds.is_empty() {
            self.repo
                .list_pending_extract_seeds(&pending_selection)
                .await?
        } else {
            input.seeds
        };
        if seeds.is_empty() {
            return Ok(LoadRunOutput { processed: 0 });
        }

        for seed in &seeds {
            let record = prepare_record(seed, &reason)?;
            self.repo.upsert_loaded(&record).await?;
            self.repo
                .mark_extract_seed_loaded(&record.dedupe_key)
                .await?;
        }

        Ok(LoadRunOutput {
            processed: seeds.len(),
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

fn normalized_pending_selection(
    platform_filter: Option<String>,
    pending_limit: usize,
) -> Result<LoadPendingSelection> {
    if pending_limit == 0 {
        return Err(anyhow!("load.pending_limit must be > 0"));
    }

    let platform_filter = platform_filter.and_then(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    });

    Ok(LoadPendingSelection {
        platform_filter,
        limit: pending_limit,
    })
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
