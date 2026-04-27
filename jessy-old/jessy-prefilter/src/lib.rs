use std::{collections::HashSet, future::Future};

use anyhow::{anyhow, Result};
use jessy_core::JobStage;

pub const STEP_STAGE: JobStage = JobStage::Prefilter;

#[derive(Debug, Clone)]
pub struct PrefilterRunInput {
    pub platform_filter: Option<String>,
    pub limit: usize,
    pub reason: String,
    pub avoid_words_in_title: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PrefilterRunOutput {
    pub selected: usize,
    pub processed: usize,
    pub passed: usize,
    pub rejected: usize,
}

#[derive(Debug, Clone)]
pub struct PrefilterSelection {
    pub platform_filter: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct PrefilterCandidate {
    pub id: i64,
    pub platform: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefilterDecisionKind {
    Passed,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefilterDecision {
    pub kind: PrefilterDecisionKind,
    pub reason_code: String,
}

#[derive(Debug, Clone)]
pub struct PrefilterTransition {
    pub id: i64,
    pub current_stage: JobStage,
    pub status_meta: String,
}

pub trait PrefilterRepo: Send + Sync {
    fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send;
    fn list_load_ready<'a>(
        &'a self,
        selection: &'a PrefilterSelection,
    ) -> impl Future<Output = Result<Vec<PrefilterCandidate>>> + Send + 'a;
    fn apply_prefilter_transition<'a>(
        &'a self,
        transition: &'a PrefilterTransition,
    ) -> impl Future<Output = Result<bool>> + Send + 'a;
}

pub struct PrefilterService<R: PrefilterRepo> {
    repo: R,
}

impl<R: PrefilterRepo> PrefilterService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub async fn run(&self, input: PrefilterRunInput) -> Result<PrefilterRunOutput> {
        let selection = normalized_selection(input.platform_filter, input.limit)?;
        let reason = normalized_reason(&input.reason);
        let avoid_words = normalized_avoid_words(input.avoid_words_in_title);
        self.repo.ensure_ready().await?;

        let candidates = self.repo.list_load_ready(&selection).await?;
        let mut processed = 0usize;
        let mut passed = 0usize;
        let mut rejected = 0usize;

        for candidate in &candidates {
            let decision = evaluate_title(&candidate.title, &avoid_words);
            if decision.kind == PrefilterDecisionKind::Passed {
                passed += 1;
            } else {
                rejected += 1;
            }
            let transition = prepare_transition(candidate, &decision, &reason);
            if self.repo.apply_prefilter_transition(&transition).await? {
                processed += 1;
            }
        }

        Ok(PrefilterRunOutput {
            selected: candidates.len(),
            processed,
            passed,
            rejected,
        })
    }
}

pub const fn step_name() -> &'static str {
    STEP_STAGE.as_str()
}

pub fn evaluate_title(title: &str, avoid_words_in_title: &[String]) -> PrefilterDecision {
    let normalized_title = title.trim().to_ascii_lowercase();
    if normalized_title.is_empty() {
        return PrefilterDecision {
            kind: PrefilterDecisionKind::Rejected,
            reason_code: "title_missing".to_string(),
        };
    }

    for avoid in avoid_words_in_title {
        if normalized_title.contains(avoid) {
            return PrefilterDecision {
                kind: PrefilterDecisionKind::Rejected,
                reason_code: format!("title_excluded:{avoid}"),
            };
        }
    }

    PrefilterDecision {
        kind: PrefilterDecisionKind::Passed,
        reason_code: "passed".to_string(),
    }
}

pub fn prepare_transition(
    candidate: &PrefilterCandidate,
    decision: &PrefilterDecision,
    reason: &str,
) -> PrefilterTransition {
    let platform = normalized_platform(&candidate.platform).unwrap_or_else(|| "unknown".to_string());
    PrefilterTransition {
        id: candidate.id,
        current_stage: STEP_STAGE,
        status_meta: format!(
            "{}:{}:{}:{}",
            step_name(),
            platform,
            decision.reason_code,
            normalized_reason(reason)
        ),
    }
}

fn normalized_selection(platform_filter: Option<String>, limit: usize) -> Result<PrefilterSelection> {
    if limit == 0 {
        return Err(anyhow!("prefilter.limit must be > 0"));
    }
    Ok(PrefilterSelection {
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
        "manual_prefilter".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_avoid_words(avoid_words: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for word in avoid_words {
        let normalized = word.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_name_is_prefilter() {
        assert_eq!(step_name(), "prefilter");
    }

    #[test]
    fn evaluate_title_rejects_missing_title() {
        let d = evaluate_title("   ", &[]);
        assert_eq!(d.kind, PrefilterDecisionKind::Rejected);
        assert_eq!(d.reason_code, "title_missing");
    }

    #[test]
    fn evaluate_title_rejects_avoid_word() {
        let d = evaluate_title("Senior Intern", &["intern".to_string()]);
        assert_eq!(d.kind, PrefilterDecisionKind::Rejected);
        assert_eq!(d.reason_code, "title_excluded:intern");
    }

    #[test]
    fn evaluate_title_passes() {
        let d = evaluate_title("Software Engineer", &["intern".to_string()]);
        assert_eq!(d.kind, PrefilterDecisionKind::Passed);
        assert_eq!(d.reason_code, "passed");
    }
}
