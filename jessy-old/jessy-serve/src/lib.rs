use std::future::Future;

use anyhow::{anyhow, Result};
use jessy_core::JobStage;

pub const STEP_STAGE: JobStage = JobStage::Serve;

#[derive(Debug, Clone)]
pub struct ServeRunInput {
    pub platform_filter: Option<String>,
    pub limit: usize,
    pub query: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ServeRunOutput {
    pub total: usize,
    pub matched: usize,
    pub rows: Vec<ServeRow>,
}

#[derive(Debug, Clone)]
pub struct ServeSelection {
    pub platform_filter: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct ServeRow {
    pub id: i64,
    pub platform: String,
    pub title: String,
    pub company: String,
    pub canonical_url: String,
    pub status_meta: String,
    pub company_summary: String,
    pub description: String,
}

pub trait ServeRepo: Send + Sync {
    fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send;
    fn list_enriched<'a>(
        &'a self,
        selection: &'a ServeSelection,
    ) -> impl Future<Output = Result<Vec<ServeRow>>> + Send + 'a;
}

pub trait ServeChannel: Send + Sync {
    fn publish<'a>(
        &'a self,
        output: &'a ServeRunOutput,
    ) -> impl Future<Output = Result<()>> + Send + 'a;
}

pub struct ServeService<R: ServeRepo, C: ServeChannel> {
    repo: R,
    channel: C,
}

impl<R: ServeRepo, C: ServeChannel> ServeService<R, C> {
    pub fn new(repo: R, channel: C) -> Self {
        Self { repo, channel }
    }

    pub async fn run(&self, input: ServeRunInput) -> Result<ServeRunOutput> {
        let selection = normalized_selection(input.platform_filter, input.limit)?;
        let query = normalized_query(input.query);
        self.repo.ensure_ready().await?;

        let rows = self.repo.list_enriched(&selection).await?;
        let total = rows.len();
        let filtered = match query {
            Some(q) => rows
                .into_iter()
                .filter(|row| row_matches_query(row, &q))
                .collect::<Vec<_>>(),
            None => rows,
        };

        let output = ServeRunOutput {
            total,
            matched: filtered.len(),
            rows: filtered,
        };
        self.channel.publish(&output).await?;
        Ok(output)
    }
}

pub const fn step_name() -> &'static str {
    STEP_STAGE.as_str()
}

fn normalized_selection(platform_filter: Option<String>, limit: usize) -> Result<ServeSelection> {
    if limit == 0 {
        return Err(anyhow!("serve.limit must be > 0"));
    }
    Ok(ServeSelection {
        platform_filter: platform_filter.and_then(|v| normalized_platform(&v)),
        limit,
    })
}

fn normalized_platform(platform: &str) -> Option<String> {
    let v = platform.trim().to_ascii_lowercase();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn normalized_query(query: Option<String>) -> Option<String> {
    query.and_then(|v| {
        let normalized = v.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn row_matches_query(row: &ServeRow, query: &str) -> bool {
    row.platform.to_ascii_lowercase().contains(query)
        || row.title.to_ascii_lowercase().contains(query)
        || row.company.to_ascii_lowercase().contains(query)
        || row.canonical_url.to_ascii_lowercase().contains(query)
        || row.status_meta.to_ascii_lowercase().contains(query)
        || row.company_summary.to_ascii_lowercase().contains(query)
        || row.description.to_ascii_lowercase().contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> ServeRow {
        ServeRow {
            id: 1,
            platform: "linkedin".to_string(),
            title: "Software Engineer".to_string(),
            company: "ACME".to_string(),
            canonical_url: "https://example.com/jobs/1".to_string(),
            status_meta: "enrich:linkedin:enriched:smoke".to_string(),
            company_summary: "Good company".to_string(),
            description: "Rust backend role".to_string(),
        }
    }

    #[test]
    fn step_name_is_serve() {
        assert_eq!(step_name(), "serve");
    }

    #[test]
    fn query_matches_title() {
        assert!(row_matches_query(&sample_row(), "software engineer"));
    }

    #[test]
    fn zero_limit_rejected() {
        let err = normalized_selection(None, 0).expect_err("expected error");
        assert!(err.to_string().contains("serve.limit"));
    }
}
