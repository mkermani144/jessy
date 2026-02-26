use std::{path::Path, str::FromStr};

use anyhow::{Context, Result};
use chrono::Utc;
use jessy_enrich::{EnrichCandidate, EnrichRepo, EnrichSelection, EnrichTransition};
use jessy_load::{LoadPreparedRecord, LoadRepo};
use jessy_prefilter::{PrefilterCandidate, PrefilterRepo, PrefilterSelection, PrefilterTransition};
use jessy_serve::{ServeRepo, ServeRow, ServeSelection};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Executor, Row, SqlitePool,
};

pub struct SqliteRepo {
    db_path: String,
}

impl SqliteRepo {
    pub fn new(db_path: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    async fn connect(&self) -> Result<SqlitePool> {
        if let Some(parent) = Path::new(&self.db_path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed creating DB parent dir {}", parent.display()))?;
        }

        let url = format!("sqlite://{}", self.db_path);
        let options = SqliteConnectOptions::from_str(&url)
            .context("failed to parse sqlite connection options")?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .with_context(|| format!("failed to connect SQLite at {}", self.db_path))?;

        pool.execute("PRAGMA foreign_keys = ON;")
            .await
            .context("failed enabling foreign_keys")?;

        Ok(pool)
    }
}

impl LoadRepo for SqliteRepo {
    fn ensure_ready(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let pool = self.connect().await?;
            migrate(&pool).await
        }
    }

    fn upsert_loaded<'a>(
        &'a self,
        record: &'a LoadPreparedRecord,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a {
        async move {
            let pool = self.connect().await?;
            let now = Utc::now().to_rfc3339();

            sqlx::query(
                "INSERT INTO jobs (
                    dedupe_key, canonical_url, platform, company, title, location, language, work_mode,
                    employment_type, posted_text, compensation_text, visa_policy_text, description,
                    requirements_json, first_seen, last_seen, source_ref, source_cursor, source_tab_url,
                    source_page_index, status, status_reason, current_stage, status_meta,
                    requirements_summary, company_summary, company_size
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(dedupe_key) DO UPDATE SET
                    last_seen=excluded.last_seen,
                    platform=excluded.platform,
                    source_ref=excluded.source_ref,
                    source_cursor=excluded.source_cursor,
                    source_tab_url=excluded.source_tab_url,
                    source_page_index=excluded.source_page_index,
                    current_stage=excluded.current_stage,
                    status_meta=excluded.status_meta",
            )
            .bind(&record.dedupe_key)
            .bind(&record.canonical_url)
            .bind(&record.platform)
            .bind("Unknown Company")
            .bind("Unknown Title")
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind("Pending prefilter step")
            .bind("[]")
            .bind(&now)
            .bind(&now)
            .bind(&record.source_ref)
            .bind(&record.source_cursor)
            .bind(&record.source_ref)
            .bind(record.legacy_source_page_index)
            .bind("not_opportunity")
            .bind("Pipeline pending prefilter")
            .bind(record.current_stage.as_str())
            .bind(&record.status_meta)
            .bind("Pending prefilter")
            .bind("Pending prefilter")
            .bind(Option::<String>::None)
            .execute(&pool)
            .await
            .context("failed upserting loaded seed into jobs")?;

            Ok(())
        }
    }
}

impl PrefilterRepo for SqliteRepo {
    fn ensure_ready(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let pool = self.connect().await?;
            migrate(&pool).await
        }
    }

    fn list_load_ready<'a>(
        &'a self,
        selection: &'a PrefilterSelection,
    ) -> impl std::future::Future<Output = Result<Vec<PrefilterCandidate>>> + Send + 'a {
        async move {
            let pool = self.connect().await?;
            let platform_filter = selection.platform_filter.as_deref();
            let rows = sqlx::query(
                "SELECT id,
                        COALESCE(platform, 'unknown') AS platform,
                        COALESCE(title, '') AS title
                 FROM jobs
                 WHERE current_stage = ?
                   AND (? IS NULL OR platform = ?)
                 ORDER BY id ASC
                 LIMIT ?",
            )
            .bind("load")
            .bind(platform_filter)
            .bind(platform_filter)
            .bind(selection.limit as i64)
            .fetch_all(&pool)
            .await
            .context("failed selecting load-ready jobs")?;

            Ok(rows
                .into_iter()
                .map(|row| PrefilterCandidate {
                    id: row.get("id"),
                    platform: row.get("platform"),
                    title: row.get("title"),
                })
                .collect())
        }
    }

    fn apply_prefilter_transition<'a>(
        &'a self,
        transition: &'a PrefilterTransition,
    ) -> impl std::future::Future<Output = Result<bool>> + Send + 'a {
        async move {
            let pool = self.connect().await?;
            let now = Utc::now().to_rfc3339();
            let res = sqlx::query(
                "UPDATE jobs
                 SET current_stage = ?, status_meta = ?, last_seen = ?
                 WHERE id = ? AND current_stage = ?",
            )
            .bind(transition.current_stage.as_str())
            .bind(&transition.status_meta)
            .bind(&now)
            .bind(transition.id)
            .bind("load")
            .execute(&pool)
            .await
            .context("failed applying prefilter transition")?;

            Ok(res.rows_affected() > 0)
        }
    }
}

impl EnrichRepo for SqliteRepo {
    fn ensure_ready(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let pool = self.connect().await?;
            migrate(&pool).await
        }
    }

    fn list_prefilter_ready<'a>(
        &'a self,
        selection: &'a EnrichSelection,
    ) -> impl std::future::Future<Output = Result<Vec<EnrichCandidate>>> + Send + 'a {
        async move {
            let pool = self.connect().await?;
            let platform_filter = selection.platform_filter.as_deref();
            let rows = sqlx::query(
                "SELECT id,
                        COALESCE(platform, 'unknown') AS platform,
                        canonical_url,
                        COALESCE(title, '') AS title,
                        COALESCE(company, '') AS company,
                        COALESCE(description, '') AS description
                 FROM jobs
                 WHERE current_stage = ?
                   AND status_meta LIKE ?
                   AND (? IS NULL OR platform = ?)
                 ORDER BY id ASC
                 LIMIT ?",
            )
            .bind("prefilter")
            .bind("%:passed:%")
            .bind(platform_filter)
            .bind(platform_filter)
            .bind(selection.limit as i64)
            .fetch_all(&pool)
            .await
            .context("failed selecting prefilter-ready jobs")?;

            Ok(rows
                .into_iter()
                .map(|row| EnrichCandidate {
                    id: row.get("id"),
                    platform: row.get("platform"),
                    canonical_url: row.get("canonical_url"),
                    title: row.get("title"),
                    company: row.get("company"),
                    description: row.get("description"),
                })
                .collect())
        }
    }

    fn apply_enrich_transition<'a>(
        &'a self,
        transition: &'a EnrichTransition,
    ) -> impl std::future::Future<Output = Result<bool>> + Send + 'a {
        async move {
            let pool = self.connect().await?;
            let now = Utc::now().to_rfc3339();
            let res = sqlx::query(
                "UPDATE jobs
                 SET current_stage = ?,
                     status_meta = ?,
                     company_summary = ?,
                     description = COALESCE(?, description),
                     last_seen = ?
                 WHERE id = ? AND current_stage = ?",
            )
            .bind(transition.current_stage.as_str())
            .bind(&transition.status_meta)
            .bind(&transition.company_summary)
            .bind(&transition.description)
            .bind(&now)
            .bind(transition.id)
            .bind("prefilter")
            .execute(&pool)
            .await
            .context("failed applying enrich transition")?;

            Ok(res.rows_affected() > 0)
        }
    }
}

impl ServeRepo for SqliteRepo {
    fn ensure_ready(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let pool = self.connect().await?;
            migrate(&pool).await
        }
    }

    fn list_enriched<'a>(
        &'a self,
        selection: &'a ServeSelection,
    ) -> impl std::future::Future<Output = Result<Vec<ServeRow>>> + Send + 'a {
        async move {
            let pool = self.connect().await?;
            let platform_filter = selection.platform_filter.as_deref();
            let rows = sqlx::query(
                "SELECT id,
                        COALESCE(platform, 'unknown') AS platform,
                        COALESCE(title, '') AS title,
                        COALESCE(company, '') AS company,
                        canonical_url,
                        COALESCE(status_meta, '') AS status_meta,
                        COALESCE(company_summary, '') AS company_summary,
                        COALESCE(description, '') AS description
                 FROM jobs
                 WHERE current_stage = ?
                   AND (? IS NULL OR platform = ?)
                 ORDER BY id DESC
                 LIMIT ?",
            )
            .bind("enrich")
            .bind(platform_filter)
            .bind(platform_filter)
            .bind(selection.limit as i64)
            .fetch_all(&pool)
            .await
            .context("failed selecting enrich-ready jobs for serve")?;

            Ok(rows
                .into_iter()
                .map(|row| ServeRow {
                    id: row.get("id"),
                    platform: row.get("platform"),
                    title: row.get("title"),
                    company: row.get("company"),
                    canonical_url: row.get("canonical_url"),
                    status_meta: row.get("status_meta"),
                    company_summary: row.get("company_summary"),
                    description: row.get("description"),
                })
                .collect())
        }
    }
}

async fn migrate(pool: &SqlitePool) -> Result<()> {
    let sql = include_str!("../../migrations/001_init.sql");

    for statement in sql.split(";\n") {
        let stmt = statement.trim();
        if stmt.is_empty() {
            continue;
        }

        pool.execute(stmt)
            .await
            .with_context(|| format!("failed executing migration statement: {stmt}"))?;
    }

    ensure_jobs_column(pool, "platform", "TEXT").await?;
    ensure_jobs_column(pool, "source_ref", "TEXT").await?;
    ensure_jobs_column(pool, "source_cursor", "TEXT").await?;
    ensure_jobs_column(pool, "current_stage", "TEXT").await?;
    ensure_jobs_column(pool, "status_meta", "TEXT").await?;
    Ok(())
}

async fn ensure_jobs_column(pool: &SqlitePool, column: &str, sqlite_type: &str) -> Result<()> {
    let rows = sqlx::query("PRAGMA table_info(jobs)")
        .fetch_all(pool)
        .await
        .context("failed to inspect jobs schema")?;
    let exists = rows
        .iter()
        .any(|r| r.get::<String, _>("name").eq_ignore_ascii_case(column));
    if exists {
        return Ok(());
    }

    let alter = format!("ALTER TABLE jobs ADD COLUMN {column} {sqlite_type}");
    pool.execute(alter.as_str())
        .await
        .with_context(|| format!("failed adding jobs.{column} column"))?;
    Ok(())
}
