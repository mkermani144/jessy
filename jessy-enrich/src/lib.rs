use std::future::Future;

use anyhow::{anyhow, Result};
use jessy_core::JobStage;

pub const STEP_STAGE: JobStage = JobStage::Enrich;

#[derive(Debug, Clone)]
pub struct EnrichRunInput {
    pub platform_filter: Option<String>,
    pub limit: usize,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct EnrichRunOutput {
    pub selected: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Clone)]
pub struct EnrichSelection {
    pub platform_filter: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct EnrichCandidate {
    pub id: i64,
    pub platform: String,
    pub canonical_url: String,
    pub title: String,
    pub company: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct EnrichTransition {
    pub id: i64,
    pub current_stage: JobStage,
    pub status_meta: String,
    pub company_summary: String,
    pub description: Option<String>,
}

pub trait EnrichRepo: Send + Sync {
    fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send;
    fn list_prefilter_ready<'a>(
        &'a self,
        selection: &'a EnrichSelection,
    ) -> impl Future<Output = Result<Vec<EnrichCandidate>>> + Send + 'a;
    fn apply_enrich_transition<'a>(
        &'a self,
        transition: &'a EnrichTransition,
    ) -> impl Future<Output = Result<bool>> + Send + 'a;
}

pub trait UrlFetcher: Send + Sync {
    fn fetch_text<'a>(&'a self, url: &'a str) -> impl Future<Output = Result<String>> + Send + 'a;
}

pub trait SummaryGenerator: Send + Sync {
    fn summarize<'a>(
        &'a self,
        candidate: &'a EnrichCandidate,
        fetched_text: Option<&'a str>,
    ) -> impl Future<Output = Result<String>> + Send + 'a;
}

pub struct EnrichService<R: EnrichRepo, F: UrlFetcher, S: SummaryGenerator> {
    repo: R,
    fetcher: F,
    summarizer: S,
}

impl<R: EnrichRepo, F: UrlFetcher, S: SummaryGenerator> EnrichService<R, F, S> {
    pub fn new(repo: R, fetcher: F, summarizer: S) -> Self {
        Self {
            repo,
            fetcher,
            summarizer,
        }
    }

    pub async fn run(&self, input: EnrichRunInput) -> Result<EnrichRunOutput> {
        let selection = normalized_selection(input.platform_filter, input.limit)?;
        let reason = normalized_reason(&input.reason);
        self.repo.ensure_ready().await?;

        let candidates = self.repo.list_prefilter_ready(&selection).await?;
        let mut processed = 0usize;
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        for candidate in &candidates {
            let fetched = self.fetcher.fetch_text(&candidate.canonical_url).await.ok();
            let fetched_text = fetched.as_deref();
            let summary = self.summarizer.summarize(candidate, fetched_text).await;

            let transition = build_transition(candidate, fetched_text, summary.as_ref(), &reason);
            if self.repo.apply_enrich_transition(&transition).await? {
                processed += 1;
                if summary.is_ok() {
                    succeeded += 1;
                } else {
                    failed += 1;
                }
            }
        }

        Ok(EnrichRunOutput {
            selected: candidates.len(),
            processed,
            succeeded,
            failed,
        })
    }
}

pub const fn step_name() -> &'static str {
    STEP_STAGE.as_str()
}

pub fn build_transition(
    candidate: &EnrichCandidate,
    fetched_text: Option<&str>,
    summary: Result<&String, &anyhow::Error>,
    reason: &str,
) -> EnrichTransition {
    let platform = normalized_platform(&candidate.platform).unwrap_or_else(|| "unknown".to_string());
    let normalized_reason = normalized_reason(reason);
    let summary_state = if summary.is_ok() {
        if fetched_text.is_some() {
            "enriched"
        } else {
            "enriched_no_fetch"
        }
    } else {
        "summary_failed"
    };

    let company_summary = match summary {
        Ok(text) => text.to_string(),
        Err(_) => "Summary unavailable".to_string(),
    };
    let description = fetched_text
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| truncate(v, 1200));

    EnrichTransition {
        id: candidate.id,
        current_stage: STEP_STAGE,
        status_meta: format!(
            "{}:{}:{}:{}",
            step_name(),
            platform,
            summary_state,
            normalized_reason
        ),
        company_summary,
        description,
    }
}

fn normalized_selection(platform_filter: Option<String>, limit: usize) -> Result<EnrichSelection> {
    if limit == 0 {
        return Err(anyhow!("enrich.limit must be > 0"));
    }
    Ok(EnrichSelection {
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

fn normalized_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        "manual_enrich".to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_name_is_enrich() {
        assert_eq!(step_name(), "enrich");
    }

    #[test]
    fn build_transition_success_with_fetch() {
        let c = EnrichCandidate {
            id: 1,
            platform: "linkedin".to_string(),
            canonical_url: "https://example.com".to_string(),
            title: "Engineer".to_string(),
            company: "ACME".to_string(),
            description: "x".to_string(),
        };
        let summary = "Good summary".to_string();
        let t = build_transition(&c, Some("fetched body"), Ok(&summary), "smoke");
        assert_eq!(t.current_stage, JobStage::Enrich);
        assert_eq!(t.status_meta, "enrich:linkedin:enriched:smoke");
        assert_eq!(t.company_summary, "Good summary");
        assert!(t.description.is_some());
    }

    #[test]
    fn build_transition_failure_marks_summary_failed() {
        let c = EnrichCandidate {
            id: 2,
            platform: "telegram".to_string(),
            canonical_url: "https://t.me/x".to_string(),
            title: "Engineer".to_string(),
            company: "ACME".to_string(),
            description: "x".to_string(),
        };
        let err = anyhow!("boom");
        let t = build_transition(&c, None, Err(&err), "smoke");
        assert_eq!(t.status_meta, "enrich:telegram:summary_failed:smoke");
        assert_eq!(t.company_summary, "Summary unavailable");
    }
}
