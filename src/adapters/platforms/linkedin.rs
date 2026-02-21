use anyhow::Result;
use futures_util::{future::BoxFuture, FutureExt};

use crate::domain::job::{JobDetailData, PlatformKind, SearchCardData, SearchPageData};
use crate::ports::browser::BrowserSession;
use crate::ports::platform::{JobPageExtractor, PlatformAdapter, SearchPageExtractor};

use super::linkedin_extract;

/// LinkedIn extractor implementation.
pub struct LinkedInAdapter;

impl LinkedInAdapter {
    /// Creates stateless LinkedIn adapter.
    pub const fn new() -> Self {
        Self
    }
}

impl SearchPageExtractor for LinkedInAdapter {
    fn extract_search<'a>(
        &'a self,
        session: &'a mut dyn BrowserSession,
    ) -> BoxFuture<'a, Result<SearchPageData>> {
        async move {
            let snapshot = linkedin_extract::extract_search_snapshot(session).await?;
            Ok(SearchPageData {
                job_cards: snapshot
                    .job_cards
                    .into_iter()
                    .map(|c| SearchCardData {
                        title: c.title,
                        job_url: c.job_url,
                    })
                    .collect(),
                job_links: snapshot.job_links,
                next_page_url: snapshot.next_page_url,
                fingerprint_source: snapshot.fingerprint_source,
            })
        }
        .boxed()
    }
}

impl JobPageExtractor for LinkedInAdapter {
    fn extract_job_detail<'a>(
        &'a self,
        session: &'a mut dyn BrowserSession,
    ) -> BoxFuture<'a, Result<JobDetailData>> {
        async move {
            let snapshot = linkedin_extract::extract_job_detail_snapshot(session).await?;
            Ok(JobDetailData {
                url: snapshot.url,
                about_job_dom: snapshot.about_job_dom,
                title: snapshot.title,
                company: snapshot.company,
                location: snapshot.location,
                employment_type: snapshot.employment_type,
                posted_text: snapshot.posted_text,
                description: snapshot.description,
                requirements: snapshot.requirements,
                company_domain: snapshot.company_domain,
                company_summary: snapshot.company_summary,
                company_size: snapshot.company_size,
            })
        }
        .boxed()
    }
}

impl PlatformAdapter for LinkedInAdapter {
    fn kind(&self) -> PlatformKind {
        PlatformKind::LinkedIn
    }

    fn matches_url(&self, url: &str) -> bool {
        let u = url.to_ascii_lowercase();
        u.contains("linkedin.com/jobs")
    }

    fn is_search_page(&self, url: &str) -> bool {
        let u = url.to_ascii_lowercase();
        u.contains("linkedin.com/jobs/search")
    }
}
