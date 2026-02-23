use std::{collections::HashSet, fs, path::Path};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub chrome: ChromeConfig,
    #[serde(default)]
    pub sources: SourcesConfig,
    pub crawl: CrawlConfig,
    pub filters: FiltersConfig,
    #[serde(default)]
    pub openai: OpenAiConfig,
    pub retention: RetentionConfig,
    pub report: ReportConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChromeConfig {
    pub debug_port: u16,
    pub profile_dir: String,
    #[serde(default = "default_true")]
    pub auto_launch: bool,
    #[serde(default = "default_chrome_binary")]
    pub binary_path: String,
    #[serde(default = "default_startup_urls")]
    pub startup_urls: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SourcesConfig {
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub allowed_keywords: Vec<String>,
    #[serde(default)]
    pub denied_domains: Vec<String>,
    #[serde(default)]
    pub denied_keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrawlConfig {
    #[serde(default = "default_max_pages")]
    pub max_pages_per_search_tab: usize,
    #[serde(default = "default_true")]
    pub stop_on_repeat_pages: bool,
    #[serde(default = "default_request_delay_ms")]
    pub request_delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FiltersConfig {
    #[serde(default)]
    pub words_to_avoid_in_title: Vec<String>,
    #[serde(default = "default_recent_posted_within_days")]
    pub recent_posted_within_days: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAiConfig {
    #[serde(default = "default_openai_url")]
    pub base_url: String,
    #[serde(default = "default_openai_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_openai_api_key_env")]
    pub api_key_env: Option<String>,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            base_url: default_openai_url(),
            model: default_openai_model(),
            api_key: None,
            api_key_env: default_openai_api_key_env(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetentionConfig {
    #[serde(default = "default_retention_days")]
    pub days: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReportConfig {
    #[serde(default = "default_terminal_style")]
    pub terminal_style: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let mut cfg: AppConfig =
            serde_yaml::from_str(&raw).context("failed to parse YAML config")?;

        cfg.normalize();
        cfg.validate()?;
        Ok(cfg)
    }

    fn normalize(&mut self) {
        self.chrome.startup_urls = normalize_urls(&self.chrome.startup_urls);
        self.sources.allowed_domains = normalize_list(&self.sources.allowed_domains);
        self.sources.allowed_keywords = normalize_list(&self.sources.allowed_keywords);
        self.sources.denied_domains = normalize_list(&self.sources.denied_domains);
        self.sources.denied_keywords = normalize_list(&self.sources.denied_keywords);

        self.filters.words_to_avoid_in_title =
            normalize_list(&self.filters.words_to_avoid_in_title);

        if let Some(v) = self.openai.api_key.take() {
            let trimmed = v.trim().to_string();
            self.openai.api_key = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }
        if let Some(v) = self.openai.api_key_env.take() {
            let trimmed = v.trim().to_string();
            self.openai.api_key_env = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }
    }

    fn validate(&self) -> Result<()> {
        if self.crawl.max_pages_per_search_tab == 0 {
            bail!("crawl.max_pages_per_search_tab must be > 0");
        }

        if self.retention.days <= 0 {
            bail!("retention.days must be > 0");
        }
        if self.filters.recent_posted_within_days == 0 {
            bail!("filters.recent_posted_within_days must be > 0");
        }

        if self.openai.model.trim().is_empty() {
            bail!("openai.model must not be empty");
        }
        if self.openai.base_url.trim().is_empty() {
            bail!("openai.base_url must not be empty");
        }
        if self.openai.api_key.is_none() && self.openai.api_key_env.is_none() {
            bail!("set one of openai.api_key or openai.api_key_env");
        }

        Ok(())
    }
}

fn normalize_list(items: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for item in items {
        let normalized = item.trim().to_ascii_lowercase();
        if !normalized.is_empty() && seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }

    out
}

fn normalize_urls(items: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for item in items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
    }

    out
}

const fn default_true() -> bool {
    true
}

fn default_chrome_binary() -> String {
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string()
}

fn default_startup_urls() -> Vec<String> {
    vec![
        "https://www.linkedin.com/jobs/search/".to_string(),
        "https://web.telegram.org/".to_string(),
    ]
}

const fn default_max_pages() -> usize {
    5
}

const fn default_request_delay_ms() -> u64 {
    1500
}

const fn default_recent_posted_within_days() -> u64 {
    1
}

fn default_openai_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_openai_api_key_env() -> Option<String> {
    Some("OPENAI_API_KEY".to_string())
}

const fn default_retention_days() -> i64 {
    90
}

fn default_terminal_style() -> String {
    "verbose_cards".to_string()
}

fn default_db_path() -> String {
    "data/jessy.db".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_lists() {
        let out = normalize_list(&[
            " LinkedIn.com ".to_string(),
            "linkedin.com".to_string(),
            "".to_string(),
        ]);
        assert_eq!(out, vec!["linkedin.com"]);
    }
}
