use std::{collections::HashSet, time::Duration};

use anyhow::{Context, Result};
use jessy_extract::{ExtractCrawler, ExtractPage, ExtractPageRequest};
use regex::Regex;
use url::Url;

pub struct HttpExtractCrawler {
    client: reqwest::Client,
}

impl HttpExtractCrawler {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("jessy/0.1")
            .build()
            .context("failed building extract reqwest client")?;
        Ok(Self { client })
    }
}

impl ExtractCrawler for HttpExtractCrawler {
    fn fetch_page<'a>(
        &'a self,
        request: &'a ExtractPageRequest,
    ) -> impl std::future::Future<Output = Result<ExtractPage>> + Send + 'a {
        async move {
            let current_url = request
                .source_cursor
                .clone()
                .unwrap_or_else(|| request.source_ref.clone());

            let response = self
                .client
                .get(&current_url)
                .send()
                .await
                .with_context(|| format!("failed fetching extract source page {current_url}"))?;
            let body = response
                .text()
                .await
                .context("failed reading extract source page response")?;

            let canonical_urls =
                extract_job_urls(&body, &current_url, &request.platform, request.max_links);
            let next_cursor = extract_next_cursor(&body, &current_url, &request.platform);

            Ok(ExtractPage {
                canonical_urls,
                next_cursor,
            })
        }
    }
}

fn extract_job_urls(body: &str, base_url: &str, platform: &str, max_links: usize) -> Vec<String> {
    let href_re = Regex::new("(?i)href\\s*=\\s*[\"']([^\"']+)[\"']").expect("valid href regex");
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for cap in href_re.captures_iter(body) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        let url = absolutize_url(raw, base_url);
        if url.is_empty() || !url.starts_with("http") {
            continue;
        }
        if !matches_platform(platform, &url) {
            continue;
        }
        if !looks_like_job_link(platform, &url) {
            continue;
        }
        if seen.insert(url.clone()) {
            out.push(url);
            if out.len() >= max_links {
                break;
            }
        }
    }

    out
}

fn extract_next_cursor(body: &str, current_url: &str, platform: &str) -> Option<String> {
    let anchor_re = Regex::new("(?is)<a\\b[^>]*>").expect("valid anchor regex");
    for cap in anchor_re.captures_iter(body) {
        let tag = cap.get(0).map(|m| m.as_str()).unwrap_or_default();
        let rel = extract_tag_attr(tag, "rel");
        if !rel
            .to_ascii_lowercase()
            .split_whitespace()
            .any(|v| v == "next")
        {
            continue;
        }
        let href = extract_tag_attr(tag, "href");
        let next_url = absolutize_url(&href, current_url);
        if !next_url.is_empty() && next_url != current_url {
            return Some(next_url);
        }
    }

    if platform.eq_ignore_ascii_case("linkedin") {
        return linkedin_next_cursor(current_url);
    }

    None
}

fn absolutize_url(raw: &str, base_url: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("javascript:") {
        return String::new();
    }

    if let Ok(url) = Url::parse(trimmed) {
        return url.to_string();
    }

    if let Ok(base) = Url::parse(base_url) {
        if let Ok(joined) = base.join(trimmed) {
            return joined.to_string();
        }
    }

    String::new()
}

fn matches_platform(platform: &str, url: &str) -> bool {
    let p = platform.trim().to_ascii_lowercase();
    let u = url.to_ascii_lowercase();
    match p.as_str() {
        "linkedin" => u.contains("linkedin.com"),
        "telegram" => u.contains("t.me/") || u.contains("telegram.me/"),
        _ => true,
    }
}

fn extract_tag_attr(tag: &str, attr: &str) -> String {
    let pattern = format!("(?i)\\b{attr}\\s*=\\s*[\"']([^\"']+)[\"']");
    let attr_re = Regex::new(&pattern).expect("valid attribute regex");
    attr_re
        .captures(tag)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

fn looks_like_job_link(platform: &str, url: &str) -> bool {
    let p = platform.trim().to_ascii_lowercase();
    let u = url.to_ascii_lowercase();
    match p.as_str() {
        "linkedin" => u.contains("/jobs/view"),
        "telegram" => u.contains("t.me/") || u.contains("telegram.me/"),
        _ => u.contains("job") || u.contains("career") || u.contains("position"),
    }
}

fn linkedin_next_cursor(current_url: &str) -> Option<String> {
    let mut url = Url::parse(current_url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !host.contains("linkedin.com") {
        return None;
    }

    let mut start = 0usize;
    let mut params = Vec::new();
    for (k, v) in url.query_pairs() {
        if k == "start" {
            start = v.parse::<usize>().ok().unwrap_or(0);
            continue;
        }
        params.push((k.to_string(), v.to_string()));
    }

    let next_start = start + 25;
    params.push(("start".to_string(), next_start.to_string()));
    url.query_pairs_mut().clear().extend_pairs(params);

    let next = url.to_string();
    if next == current_url {
        None
    } else {
        Some(next)
    }
}
