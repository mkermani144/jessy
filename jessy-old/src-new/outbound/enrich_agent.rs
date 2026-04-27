use std::time::Duration;

use anyhow::{Context, Result};
use jessy_enrich::{EnrichCandidate, SummaryGenerator, UrlFetcher};
use regex::Regex;

pub struct HttpUrlFetcher {
    client: reqwest::Client,
}

impl HttpUrlFetcher {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("jessy/0.1")
            .build()
            .context("failed building reqwest client")?;
        Ok(Self { client })
    }
}

impl UrlFetcher for HttpUrlFetcher {
    fn fetch_text<'a>(
        &'a self,
        url: &'a str,
    ) -> impl std::future::Future<Output = Result<String>> + Send + 'a {
        async move {
            let response = self
                .client
                .get(url)
                .send()
                .await
                .with_context(|| format!("failed fetching url {url}"))?;
            let body = response.text().await.context("failed reading response text")?;
            Ok(clean_html(&body))
        }
    }
}

pub struct HeuristicSummaryGenerator;

impl HeuristicSummaryGenerator {
    pub const fn new() -> Self {
        Self
    }
}

impl SummaryGenerator for HeuristicSummaryGenerator {
    fn summarize<'a>(
        &'a self,
        candidate: &'a EnrichCandidate,
        fetched_text: Option<&'a str>,
    ) -> impl std::future::Future<Output = Result<String>> + Send + 'a {
        async move {
            let base = match fetched_text.map(str::trim).filter(|v| !v.is_empty()) {
                Some(text) => truncate(text, 260),
                None => truncate(candidate.description.trim(), 260),
            };

            let title = if candidate.title.trim().is_empty() {
                "Unknown role"
            } else {
                candidate.title.trim()
            };
            let company = if candidate.company.trim().is_empty() {
                "Unknown company"
            } else {
                candidate.company.trim()
            };
            let sentence = if base.is_empty() {
                format!("{title} at {company}. Summary unavailable from source content.")
            } else {
                format!("{title} at {company}. {base}")
            };
            Ok(sentence)
        }
    }
}

fn clean_html(input: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").expect("valid html strip regex");
    let spaced = tag_re.replace_all(input, " ");
    let ws_re = Regex::new(r"\s+").expect("valid whitespace regex");
    let collapsed = ws_re.replace_all(&spaced, " ");
    collapsed.trim().to_string()
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}
