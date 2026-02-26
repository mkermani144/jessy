use std::{path::Path, str::FromStr};

use anyhow::{Context, Result};
use chrono::Utc;
use jessy_extract::{ExtractPreparedRecord, ExtractRepo};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Executor, Row, SqlitePool,
};

pub struct SqliteExtractRepo {
    db_path: String,
}

impl SqliteExtractRepo {
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

impl ExtractRepo for SqliteExtractRepo {
    fn ensure_ready(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let pool = self.connect().await?;
            migrate(&pool).await
        }
    }

    fn upsert_extracted<'a>(
        &'a self,
        record: &'a ExtractPreparedRecord,
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
            .bind("Pending load step")
            .bind("[]")
            .bind(&now)
            .bind(&now)
            .bind(&record.source_ref)
            .bind(&record.source_cursor)
            .bind(&record.source_ref)
            .bind(record.legacy_source_page_index)
            .bind("not_opportunity")
            .bind("Pipeline pending load")
            .bind(record.current_stage.as_str())
            .bind(&record.status_meta)
            .bind("Pending load")
            .bind("Pending load")
            .bind(Option::<String>::None)
            .execute(&pool)
            .await
            .context("failed upserting extracted seed into jobs")?;

            Ok(())
        }
    }
}

async fn migrate(pool: &SqlitePool) -> Result<()> {
    let sql = include_str!("../migrations/001_init.sql");

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
