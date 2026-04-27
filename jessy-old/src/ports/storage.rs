use anyhow::Result;
use async_trait::async_trait;

use crate::domain::job::{JobRecord, ReportRow};

#[derive(Debug, Clone)]
pub struct RunCompletion {
    pub run_id: i64,
    pub status: String,
    pub total_scanned: usize,
    pub new_jobs: usize,
    pub opportunities: usize,
    pub not_opportunities: usize,
    pub error: Option<String>,
}

/// Port for all scan persistence and retention operations.
#[async_trait]
pub trait ScanRepository: Send + Sync {
    /// Lightweight liveness check (for `doctor`).
    async fn healthcheck(&self) -> Result<()>;
    /// Creates a new run row and returns run id.
    async fn start_run(&self) -> Result<i64>;
    /// Marks run completion with metrics and optional error.
    async fn finish_run(&self, completion: &RunCompletion) -> Result<()>;
    /// Checks whether a search-page fingerprint has already been seen.
    async fn has_seen_page_fingerprint(&self, tab_key: &str, fingerprint: &str) -> Result<bool>;
    /// Records search-page fingerprint used for repeat detection.
    async fn record_page_fingerprint(
        &self,
        tab_key: &str,
        fingerprint: &str,
        page_index: i64,
    ) -> Result<()>;
    /// Checks whether a canonical job URL has already been processed.
    async fn is_canonical_url_seen(&self, canonical_url: &str) -> Result<bool>;
    /// Inserts/updates a job and run linkage.
    async fn upsert_job(&self, run_id: i64, job: &JobRecord) -> Result<(i64, bool)>;
    /// Loads report projection for one run.
    async fn load_report_rows(&self, run_id: i64) -> Result<Vec<ReportRow>>;
    /// Deletes records older than retention policy.
    async fn cleanup_old_records(&self, retention_days: i64) -> Result<u64>;
    /// Resets all scan history and dedupe state.
    async fn clear_all_history(&self) -> Result<()>;
}
