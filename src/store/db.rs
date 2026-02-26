use std::{path::Path, str::FromStr};

use anyhow::{Context, Result};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Executor, Row, SqlitePool,
};

pub async fn connect(db_path: &str) -> Result<SqlitePool> {
    if let Some(parent) = Path::new(db_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed creating DB parent dir {}", parent.display()))?;
    }

    let url = format!("sqlite://{db_path}");
    let options = SqliteConnectOptions::from_str(&url)
        .context("failed to parse sqlite connection options")?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to connect SQLite at {db_path}"))?;

    pool.execute("PRAGMA foreign_keys = ON;")
        .await
        .context("failed enabling foreign_keys")?;

    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
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

    ensure_jobs_column(pool, "work_mode", "TEXT").await?;
    ensure_jobs_column(pool, "language", "TEXT").await?;
    ensure_jobs_column(pool, "compensation_text", "TEXT").await?;
    ensure_jobs_column(pool, "visa_policy_text", "TEXT").await?;
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
