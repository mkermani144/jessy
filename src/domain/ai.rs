use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Payload sent to the AI extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiInput {
    pub dom_element: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct AiRequirements {
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub tools: Vec<String>,
    pub databases: Vec<String>,
    pub cloud: Vec<String>,
    pub other: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkMode {
    Remote,
    Hybrid,
    OnSite,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EmploymentType {
    FullTime,
    PartTime,
    Contract,
    Internship,
    Temporary,
    Freelance,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisaPolicy {
    VisaSponsored,
    Unknown,
    VisaNotSponsored,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum CompanySize {
    #[serde(rename = "1-10")]
    OneToTen,
    #[serde(rename = "11-50")]
    ElevenToFifty,
    #[serde(rename = "51-500")]
    FiftyOneToFiveHundred,
    #[serde(rename = "above")]
    Above,
}

/// Structured extractor output used by orchestration layers.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiDecision {
    pub company_name: Option<String>,
    pub location_text: Option<String>,
    pub language: Option<String>,
    pub work_mode: Option<WorkMode>,
    pub employment_type: Option<EmploymentType>,
    pub description: Option<String>,
    pub requirements: AiRequirements,
    pub compensation_text: Option<String>,
    pub visa_policy_text: Option<VisaPolicy>,
    pub company_summary: Option<String>,
    pub company_size_text: Option<CompanySize>,
}

impl AiDecision {
    /// Normalizes whitespace and requirement lists to keep downstream behavior stable.
    pub fn sanitized(mut self) -> Self {
        self.company_name = normalize_opt(self.company_name.take());
        self.location_text = normalize_opt(self.location_text.take());
        self.language = normalize_language_opt(self.language.take());
        self.description = normalize_opt(self.description.take());
        self.compensation_text = normalize_opt(self.compensation_text.take());
        self.company_summary = normalize_opt(self.company_summary.take());

        self.requirements.languages = normalize_list(self.requirements.languages);
        self.requirements.frameworks = normalize_list(self.requirements.frameworks);
        self.requirements.tools = normalize_list(self.requirements.tools);
        self.requirements.databases = normalize_list(self.requirements.databases);
        self.requirements.cloud = normalize_list(self.requirements.cloud);
        self.requirements.other = normalize_list(self.requirements.other);

        self
    }
}

fn normalize_opt(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.split_whitespace().collect::<Vec<_>>().join(" "))
        }
    })
}

fn normalize_list(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for v in values {
        if let Some(clean) = normalize_opt(Some(v)) {
            if !out.iter().any(|x: &String| x.eq_ignore_ascii_case(&clean)) {
                out.push(clean);
            }
        }
    }
    out
}

fn normalize_language_opt(value: Option<String>) -> Option<String> {
    normalize_opt(value).map(|v| v.to_ascii_lowercase())
}
