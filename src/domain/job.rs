use serde::{Deserialize, Serialize};

/// Supported job-source platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformKind {
    LinkedIn,
}

impl PlatformKind {
    /// Stable lowercase identifier used in logs and persistence.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinkedIn => "linkedin",
        }
    }
}

/// Lightweight candidate from a search/list page.
#[derive(Debug, Clone)]
pub struct SearchCardData {
    pub title: String,
    pub job_url: String,
}

/// Parsed snapshot from a search/list page.
#[derive(Debug, Clone)]
pub struct SearchPageData {
    pub job_cards: Vec<SearchCardData>,
    pub job_links: Vec<String>,
    pub next_page_url: Option<String>,
    pub fingerprint_source: String,
}

/// Parsed snapshot from a job detail page.
#[derive(Debug, Clone)]
pub struct JobDetailData {
    pub url: String,
    pub about_job_dom: String,
    pub title: String,
    pub company: String,
    pub location: Option<String>,
    pub employment_type: Option<String>,
    pub posted_text: Option<String>,
    pub description: String,
    pub requirements: Vec<String>,
    pub company_domain: Option<String>,
    pub company_summary: Option<String>,
    pub company_size: Option<String>,
}

/// Status persisted for each job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Opportunity,
    NotOpportunity,
}

impl JobStatus {
    /// Stable lowercase identifier used in database rows.
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Opportunity => "opportunity",
            JobStatus::NotOpportunity => "not_opportunity",
        }
    }
}

/// Canonical job record written to persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub dedupe_key: String,
    pub canonical_url: String,
    pub company: String,
    pub title: String,
    pub location: Option<String>,
    pub work_mode: Option<String>,
    pub employment_type: Option<String>,
    pub posted_text: Option<String>,
    pub compensation_text: Option<String>,
    pub visa_policy_text: Option<String>,
    pub description: String,
    pub requirements: Vec<String>,
    pub source_tab_url: String,
    pub source_page_index: i64,
    pub status: JobStatus,
    pub status_reason: String,
    pub requirements_summary: String,
    pub company_summary: String,
    pub company_size: Option<String>,
}

/// Minimal projection used by terminal reporting.
#[derive(Debug, Clone)]
pub struct ReportRow {
    pub title: String,
    pub company: Option<String>,
    pub canonical_url: String,
    pub status: String,
    pub summary: String,
    pub location: Option<String>,
    pub work_mode: Option<String>,
    pub employment_type: Option<String>,
    pub posted_text: Option<String>,
    pub compensation_text: Option<String>,
    pub visa_policy_text: Option<String>,
    pub description: Option<String>,
    pub company_summary: Option<String>,
    pub company_size: Option<String>,
    pub requirements: Vec<String>,
}
