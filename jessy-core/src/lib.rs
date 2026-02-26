use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStage {
    Extract,
    Load,
    Prefilter,
    Enrich,
    Serve,
}

impl JobStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Load => "load",
            Self::Prefilter => "prefilter",
            Self::Enrich => "enrich",
            Self::Serve => "serve",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "extract" => Some(Self::Extract),
            "load" => Some(Self::Load),
            "prefilter" => Some(Self::Prefilter),
            "enrich" => Some(Self::Enrich),
            "serve" => Some(Self::Serve),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageStatusMeta {
    pub reason: String,
}

impl StageStatusMeta {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: i64,
    pub platform: String,
    pub canonical_url: String,
    pub source_ref: Option<String>,
    pub source_cursor: Option<String>,
    pub current_stage: JobStage,
    pub status_meta: Option<StageStatusMeta>,
    pub first_seen: String,
    pub last_seen: String,
}

impl Job {
    pub fn new(
        id: i64,
        platform: impl Into<String>,
        canonical_url: impl Into<String>,
        first_seen: String,
        last_seen: String,
    ) -> Self {
        Self {
            id,
            platform: platform.into(),
            canonical_url: canonical_url.into(),
            source_ref: None,
            source_cursor: None,
            current_stage: JobStage::Extract,
            status_meta: None,
            first_seen,
            last_seen,
        }
    }
}

pub trait StepService {
    type Input;
    type Output;
    type Error;

    fn run(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{Job, JobStage};

    #[test]
    fn stage_roundtrip_strings() {
        assert_eq!(JobStage::from_str(JobStage::Extract.as_str()), Some(JobStage::Extract));
        assert_eq!(JobStage::from_str(JobStage::Load.as_str()), Some(JobStage::Load));
        assert_eq!(JobStage::from_str("unknown"), None);
    }

    #[test]
    fn new_job_defaults_to_extract_stage() {
        let job = Job::new(
            42,
            "linkedin",
            "https://example.com/jobs/42",
            "2026-02-25T00:00:00Z".to_string(),
            "2026-02-25T00:00:00Z".to_string(),
        );
        assert_eq!(job.platform, "linkedin");
        assert_eq!(job.current_stage, JobStage::Extract);
        assert!(job.status_meta.is_none());
    }
}
