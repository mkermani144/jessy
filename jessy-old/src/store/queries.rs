use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, SqlitePool};

use super::models::JobRecord;
use crate::{domain::job::ReportRow, ports::storage::RunCompletion};

pub async fn start_run(pool: &SqlitePool) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let rec = sqlx::query(
        "INSERT INTO run_logs (started_at, status, total_scanned, new_jobs, opportunities, not_opportunities) VALUES (?, 'running', 0, 0, 0, 0)",
    )
    .bind(now)
    .execute(pool)
    .await
    .context("failed to start run log")?;

    Ok(rec.last_insert_rowid())
}

pub async fn finish_run(pool: &SqlitePool, completion: &RunCompletion) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE run_logs SET finished_at = ?, status = ?, total_scanned = ?, new_jobs = ?, opportunities = ?, not_opportunities = ?, error = ? WHERE id = ?",
    )
    .bind(now)
    .bind(&completion.status)
    .bind(completion.total_scanned as i64)
    .bind(completion.new_jobs as i64)
    .bind(completion.opportunities as i64)
    .bind(completion.not_opportunities as i64)
    .bind(&completion.error)
    .bind(completion.run_id)
    .execute(pool)
    .await
    .context("failed to finish run log")?;

    Ok(())
}

pub async fn has_seen_page_fingerprint(
    pool: &SqlitePool,
    tab_key: &str,
    fingerprint: &str,
) -> Result<bool> {
    let row = sqlx::query(
        "SELECT 1 FROM search_page_fingerprints WHERE tab_key = ? AND fingerprint = ? LIMIT 1",
    )
    .bind(tab_key)
    .bind(fingerprint)
    .fetch_optional(pool)
    .await
    .context("failed checking page fingerprint")?;

    Ok(row.is_some())
}

pub async fn record_page_fingerprint(
    pool: &SqlitePool,
    tab_key: &str,
    fingerprint: &str,
    page_index: i64,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO search_page_fingerprints (tab_key, fingerprint, page_index, seen_at) VALUES (?, ?, ?, ?)",
    )
    .bind(tab_key)
    .bind(fingerprint)
    .bind(page_index)
    .bind(now)
    .execute(pool)
    .await
    .context("failed inserting page fingerprint")?;

    Ok(())
}

pub async fn is_canonical_url_seen(pool: &SqlitePool, canonical_url: &str) -> Result<bool> {
    let row = sqlx::query("SELECT 1 FROM jobs WHERE canonical_url = ? LIMIT 1")
        .bind(canonical_url)
        .fetch_optional(pool)
        .await
        .context("failed checking canonical_url")?;

    Ok(row.is_some())
}

pub async fn upsert_job(pool: &SqlitePool, run_id: i64, job: &JobRecord) -> Result<(i64, bool)> {
    let now = Utc::now().to_rfc3339();

    let existing = sqlx::query("SELECT id FROM jobs WHERE dedupe_key = ?")
        .bind(&job.dedupe_key)
        .fetch_optional(pool)
        .await
        .context("failed selecting dedupe_key")?;

    let (job_id, is_new) = if let Some(row) = existing {
        let id: i64 = row.get("id");
        sqlx::query(
            "UPDATE jobs SET canonical_url=?, company=?, title=?, location=?, language=?, work_mode=?, employment_type=?, posted_text=?, compensation_text=?, visa_policy_text=?, description=?, requirements_json=?, last_seen=?, source_tab_url=?, source_page_index=?, status=?, status_reason=?, requirements_summary=?, company_summary=?, company_size=? WHERE id=?",
        )
        .bind(&job.canonical_url)
        .bind(&job.company)
        .bind(&job.title)
        .bind(&job.location)
        .bind(&job.language)
        .bind(&job.work_mode)
        .bind(&job.employment_type)
        .bind(&job.posted_text)
        .bind(&job.compensation_text)
        .bind(&job.visa_policy_text)
        .bind(&job.description)
        .bind(serde_json::to_string(&job.requirements).context("serialize requirements")?)
        .bind(&now)
        .bind(&job.source_tab_url)
        .bind(job.source_page_index)
        .bind(job.status.as_str())
        .bind(&job.status_reason)
        .bind(&job.requirements_summary)
        .bind(&job.company_summary)
        .bind(&job.company_size)
        .bind(id)
        .execute(pool)
        .await
        .context("failed updating existing job")?;
        (id, false)
    } else {
        let rec = sqlx::query(
            "INSERT INTO jobs (dedupe_key, canonical_url, company, title, location, language, work_mode, employment_type, posted_text, compensation_text, visa_policy_text, description, requirements_json, first_seen, last_seen, source_tab_url, source_page_index, status, status_reason, requirements_summary, company_summary, company_size) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&job.dedupe_key)
        .bind(&job.canonical_url)
        .bind(&job.company)
        .bind(&job.title)
        .bind(&job.location)
        .bind(&job.language)
        .bind(&job.work_mode)
        .bind(&job.employment_type)
        .bind(&job.posted_text)
        .bind(&job.compensation_text)
        .bind(&job.visa_policy_text)
        .bind(&job.description)
        .bind(serde_json::to_string(&job.requirements).context("serialize requirements")?)
        .bind(&now)
        .bind(&now)
        .bind(&job.source_tab_url)
        .bind(job.source_page_index)
        .bind(job.status.as_str())
        .bind(&job.status_reason)
        .bind(&job.requirements_summary)
        .bind(&job.company_summary)
        .bind(&job.company_size)
        .execute(pool)
        .await
        .context("failed inserting job")?;
        (rec.last_insert_rowid(), true)
    };

    sqlx::query(
        "INSERT INTO job_observations (job_id, run_id, source_tab_url, page_index, observed_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(job_id)
    .bind(run_id)
    .bind(&job.source_tab_url)
    .bind(job.source_page_index)
    .bind(&now)
    .execute(pool)
    .await
    .context("failed inserting job observation")?;

    sqlx::query("INSERT INTO run_job_results (run_id, job_id, is_new, status) VALUES (?, ?, ?, ?)")
        .bind(run_id)
        .bind(job_id)
        .bind(if is_new { 1_i64 } else { 0_i64 })
        .bind(job.status.as_str())
        .execute(pool)
        .await
        .context("failed inserting run_job_results")?;

    Ok((job_id, is_new))
}

pub async fn load_report_rows(pool: &SqlitePool, run_id: i64) -> Result<Vec<ReportRow>> {
    let rows = sqlx::query(
        "SELECT j.title, j.source_page_index, j.company, j.canonical_url, j.status, j.status_reason, j.location, j.language, j.work_mode, j.employment_type, j.posted_text, j.compensation_text, j.visa_policy_text, j.description, j.requirements_json, j.company_summary, j.company_size FROM run_job_results r JOIN jobs j ON j.id = r.job_id WHERE r.run_id = ? ORDER BY r.id ASC",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("failed loading report rows")?;

    let mut out = Vec::with_capacity(rows.len());

    for row in rows {
        let requirements_json: String = row.get("requirements_json");
        let requirements =
            serde_json::from_str::<Vec<String>>(&requirements_json).unwrap_or_default();
        out.push(ReportRow {
            title: row.get("title"),
            source_page_index: row.get("source_page_index"),
            company: row.get("company"),
            canonical_url: row.get("canonical_url"),
            status: row.get("status"),
            summary: row.get("status_reason"),
            location: row.get("location"),
            language: row.get("language"),
            work_mode: row.get("work_mode"),
            employment_type: row.get("employment_type"),
            posted_text: row.get("posted_text"),
            compensation_text: row.get("compensation_text"),
            visa_policy_text: row.get("visa_policy_text"),
            description: row.get("description"),
            company_summary: row.get("company_summary"),
            company_size: row.get("company_size"),
            requirements,
        });
    }

    Ok(out)
}
