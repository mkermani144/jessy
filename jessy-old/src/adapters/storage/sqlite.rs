use anyhow::Result;
use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::{
    domain::job::{JobRecord, ReportRow},
    ports::storage::{RunCompletion, ScanRepository},
    store::{db, queries, retention},
};

/// SQLite-backed implementation of `ScanRepository`.
pub struct SqliteScanRepository {
    pool: SqlitePool,
}

impl SqliteScanRepository {
    /// Opens DB connection and ensures schema is migrated.
    pub async fn connect(db_path: &str) -> Result<Self> {
        let pool = db::connect(db_path).await?;
        db::migrate(&pool).await?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl ScanRepository for SqliteScanRepository {
    async fn healthcheck(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn start_run(&self) -> Result<i64> {
        queries::start_run(&self.pool).await
    }

    async fn finish_run(&self, completion: &RunCompletion) -> Result<()> {
        queries::finish_run(&self.pool, completion).await
    }

    async fn has_seen_page_fingerprint(&self, tab_key: &str, fingerprint: &str) -> Result<bool> {
        queries::has_seen_page_fingerprint(&self.pool, tab_key, fingerprint).await
    }

    async fn record_page_fingerprint(
        &self,
        tab_key: &str,
        fingerprint: &str,
        page_index: i64,
    ) -> Result<()> {
        queries::record_page_fingerprint(&self.pool, tab_key, fingerprint, page_index).await
    }

    async fn is_canonical_url_seen(&self, canonical_url: &str) -> Result<bool> {
        queries::is_canonical_url_seen(&self.pool, canonical_url).await
    }

    async fn upsert_job(&self, run_id: i64, job: &JobRecord) -> Result<(i64, bool)> {
        queries::upsert_job(&self.pool, run_id, job).await
    }

    async fn load_report_rows(&self, run_id: i64) -> Result<Vec<ReportRow>> {
        queries::load_report_rows(&self.pool, run_id).await
    }

    async fn cleanup_old_records(&self, retention_days: i64) -> Result<u64> {
        retention::cleanup_old_records(&self.pool, retention_days).await
    }

    async fn clear_all_history(&self) -> Result<()> {
        retention::clear_all_history(&self.pool).await
    }
}
