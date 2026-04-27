use std::{collections::HashSet, future::Future};

use anyhow::{anyhow, Result};
use jessy_core::JobStage;

pub const STEP_STAGE: JobStage = JobStage::Extract;

#[derive(Debug, Clone)]
pub struct ExtractSource {
    pub platform: String,
    pub source_ref: String,
    pub source_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExtractRunInput {
    pub sources: Vec<ExtractSource>,
    pub max_pages_per_source: usize,
    pub max_links_per_page: usize,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ExtractRunOutput {
    pub selected_sources: usize,
    pub crawled_pages: usize,
    pub discovered: usize,
    pub emitted: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractPageRequest {
    pub platform: String,
    pub source_ref: String,
    pub source_cursor: Option<String>,
    pub max_links: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractPage {
    pub canonical_urls: Vec<String>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoadSeed {
    pub platform: String,
    pub canonical_url: String,
    pub source_ref: String,
    pub source_cursor: Option<String>,
}

pub trait ExtractCrawler: Send + Sync {
    fn fetch_page<'a>(
        &'a self,
        request: &'a ExtractPageRequest,
    ) -> impl Future<Output = Result<ExtractPage>> + Send + 'a;
}

pub trait ExtractRepo: Send + Sync {
    fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send;
    fn emit_load_seed<'a>(
        &'a self,
        seed: &'a LoadSeed,
        reason: &'a str,
    ) -> impl Future<Output = Result<bool>> + Send + 'a;
}

pub struct ExtractService<R: ExtractRepo, C: ExtractCrawler> {
    repo: R,
    crawler: C,
}

impl<R: ExtractRepo, C: ExtractCrawler> ExtractService<R, C> {
    pub fn new(repo: R, crawler: C) -> Self {
        Self { repo, crawler }
    }

    pub async fn run(&self, input: ExtractRunInput) -> Result<ExtractRunOutput> {
        let reason = normalized_reason(&input.reason);
        let sources = normalized_sources(input.sources)?;
        if input.max_pages_per_source == 0 {
            return Err(anyhow!("extract.max_pages_per_source must be > 0"));
        }
        if input.max_links_per_page == 0 {
            return Err(anyhow!("extract.max_links_per_page must be > 0"));
        }

        self.repo.ensure_ready().await?;

        let mut crawled_pages = 0usize;
        let mut discovered = 0usize;
        let mut emitted = 0usize;
        let mut seen = HashSet::new();

        for source in &sources {
            let mut cursor = source.source_cursor.clone();
            for _ in 0..input.max_pages_per_source {
                let page = self
                    .crawler
                    .fetch_page(&ExtractPageRequest {
                        platform: source.platform.clone(),
                        source_ref: source.source_ref.clone(),
                        source_cursor: cursor.clone(),
                        max_links: input.max_links_per_page,
                    })
                    .await?;
                crawled_pages += 1;

                for canonical_url in page.canonical_urls {
                    let normalized_url = normalized_url(&canonical_url);
                    if normalized_url.is_empty() {
                        continue;
                    }
                    let key = format!("{}:{}", source.platform, normalized_url);
                    if !seen.insert(key) {
                        continue;
                    }
                    discovered += 1;
                    let seed = LoadSeed {
                        platform: source.platform.clone(),
                        canonical_url: normalized_url,
                        source_ref: source.source_ref.clone(),
                        source_cursor: cursor.clone(),
                    };
                    if self.repo.emit_load_seed(&seed, &reason).await? {
                        emitted += 1;
                    }
                }

                cursor = normalized_cursor(page.next_cursor);
                if cursor.is_none() {
                    break;
                }
            }
        }

        Ok(ExtractRunOutput {
            selected_sources: sources.len(),
            crawled_pages,
            discovered,
            emitted,
        })
    }
}

pub const fn step_name() -> &'static str {
    STEP_STAGE.as_str()
}

fn normalized_sources(sources: Vec<ExtractSource>) -> Result<Vec<ExtractSource>> {
    if sources.is_empty() {
        return Err(anyhow!("extract.sources must not be empty"));
    }

    let mut out = Vec::new();
    for source in sources {
        let platform = source.platform.trim().to_ascii_lowercase();
        if platform.is_empty() {
            return Err(anyhow!("extract source platform is empty"));
        }
        let source_ref = normalized_url(&source.source_ref);
        if source_ref.is_empty() {
            return Err(anyhow!("extract source_ref is empty"));
        }
        out.push(ExtractSource {
            platform,
            source_ref,
            source_cursor: normalized_cursor(source.source_cursor),
        });
    }
    Ok(out)
}

fn normalized_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        "manual_extract".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_cursor(cursor: Option<String>) -> Option<String> {
    cursor.and_then(|value| {
        let normalized = normalized_url(&value);
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn normalized_url(url: &str) -> String {
    url.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct FakeRepo {
        emitted: Arc<Mutex<Vec<LoadSeed>>>,
    }

    impl FakeRepo {
        fn new() -> Self {
            Self {
                emitted: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl ExtractRepo for FakeRepo {
        fn ensure_ready(&self) -> impl Future<Output = Result<()>> + Send {
            async { Ok(()) }
        }

        fn emit_load_seed<'a>(
            &'a self,
            seed: &'a LoadSeed,
            _reason: &'a str,
        ) -> impl Future<Output = Result<bool>> + Send + 'a {
            async move {
                self.emitted.lock().expect("poison").push(seed.clone());
                Ok(true)
            }
        }
    }

    struct FakeCrawler;

    impl ExtractCrawler for FakeCrawler {
        fn fetch_page<'a>(
            &'a self,
            request: &'a ExtractPageRequest,
        ) -> impl Future<Output = Result<ExtractPage>> + Send + 'a {
            async move {
                let page = if request.source_cursor.is_none() {
                    ExtractPage {
                        canonical_urls: vec![
                            "https://example.com/jobs/1".to_string(),
                            "https://example.com/jobs/1".to_string(),
                        ],
                        next_cursor: Some("https://example.com/search?page=2".to_string()),
                    }
                } else {
                    ExtractPage {
                        canonical_urls: vec!["https://example.com/jobs/2".to_string()],
                        next_cursor: None,
                    }
                };
                Ok(page)
            }
        }
    }

    #[test]
    fn step_name_is_extract() {
        assert_eq!(step_name(), "extract");
    }

    #[tokio::test]
    async fn run_dedupes_and_emits() {
        let repo = FakeRepo::new();
        let service = ExtractService::new(repo.clone(), FakeCrawler);
        let out = service
            .run(ExtractRunInput {
                sources: vec![ExtractSource {
                    platform: "LinkedIn".to_string(),
                    source_ref: "https://example.com/search".to_string(),
                    source_cursor: None,
                }],
                max_pages_per_source: 3,
                max_links_per_page: 20,
                reason: "smoke".to_string(),
            })
            .await
            .expect("extract run");

        assert_eq!(out.selected_sources, 1);
        assert_eq!(out.crawled_pages, 2);
        assert_eq!(out.discovered, 2);
        assert_eq!(out.emitted, 2);
        assert_eq!(repo.emitted.lock().expect("poison").len(), 2);
    }

    #[tokio::test]
    async fn run_rejects_zero_pages() {
        let repo = FakeRepo::new();
        let service = ExtractService::new(repo, FakeCrawler);
        let err = service
            .run(ExtractRunInput {
                sources: vec![ExtractSource {
                    platform: "linkedin".to_string(),
                    source_ref: "https://example.com".to_string(),
                    source_cursor: None,
                }],
                max_pages_per_source: 0,
                max_links_per_page: 10,
                reason: "x".to_string(),
            })
            .await
            .expect_err("expected error");
        assert!(err.to_string().contains("max_pages_per_source"));
    }
}
