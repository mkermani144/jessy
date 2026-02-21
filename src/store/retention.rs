use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use sqlx::SqlitePool;

pub async fn cleanup_old_records(pool: &SqlitePool, retention_days: i64) -> Result<u64> {
    let cutoff = (Utc::now() - Duration::days(retention_days)).to_rfc3339();

    let old_job_ids = sqlx::query_scalar::<_, i64>("SELECT id FROM jobs WHERE last_seen < ?")
        .bind(&cutoff)
        .fetch_all(pool)
        .await
        .context("failed selecting old jobs")?;

    for job_id in &old_job_ids {
        sqlx::query("DELETE FROM job_observations WHERE job_id = ?")
            .bind(job_id)
            .execute(pool)
            .await
            .context("failed deleting old observations")?;

        sqlx::query("DELETE FROM run_job_results WHERE job_id = ?")
            .bind(job_id)
            .execute(pool)
            .await
            .context("failed deleting old run links")?;
    }

    let deleted = sqlx::query("DELETE FROM jobs WHERE last_seen < ?")
        .bind(&cutoff)
        .execute(pool)
        .await
        .context("failed deleting old jobs")?
        .rows_affected();

    sqlx::query("DELETE FROM run_logs WHERE started_at < ?")
        .bind(&cutoff)
        .execute(pool)
        .await
        .context("failed deleting old run logs")?;

    sqlx::query("DELETE FROM search_page_fingerprints WHERE seen_at < ?")
        .bind(&cutoff)
        .execute(pool)
        .await
        .context("failed deleting old page fingerprints")?;

    Ok(deleted)
}

pub async fn clear_all_history(pool: &SqlitePool) -> Result<()> {
    // Explicit full reset for dedupe/history without manual DB deletion.
    sqlx::query("DELETE FROM search_page_fingerprints")
        .execute(pool)
        .await
        .context("failed clearing page fingerprints")?;

    sqlx::query("DELETE FROM run_job_results")
        .execute(pool)
        .await
        .context("failed clearing run job results")?;

    sqlx::query("DELETE FROM job_observations")
        .execute(pool)
        .await
        .context("failed clearing job observations")?;

    sqlx::query("DELETE FROM jobs")
        .execute(pool)
        .await
        .context("failed clearing jobs")?;

    sqlx::query("DELETE FROM run_logs")
        .execute(pool)
        .await
        .context("failed clearing run logs")?;

    Ok(())
}
