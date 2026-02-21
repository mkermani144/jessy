use std::sync::Arc;

use anyhow::Result;
use futures_util::future::BoxFuture;

use crate::domain::job::{JobDetailData, PlatformKind, SearchPageData};
use crate::ports::browser::BrowserSession;

/// Extracts search/list page snapshots for a platform.
pub trait SearchPageExtractor: Send + Sync {
    fn extract_search<'a>(
        &'a self,
        session: &'a mut dyn BrowserSession,
    ) -> BoxFuture<'a, Result<SearchPageData>>;
}

/// Extracts job detail snapshots for a platform.
pub trait JobPageExtractor: Send + Sync {
    fn extract_job_detail<'a>(
        &'a self,
        session: &'a mut dyn BrowserSession,
    ) -> BoxFuture<'a, Result<JobDetailData>>;
}

/// Platform adapter combining URL matching and extraction behavior.
pub trait PlatformAdapter: SearchPageExtractor + JobPageExtractor + Send + Sync {
    /// Stable platform kind.
    fn kind(&self) -> PlatformKind;
    /// Returns true if URL belongs to this platform.
    fn matches_url(&self, url: &str) -> bool;
    /// Returns true if URL points to a search/list page.
    fn is_search_page(&self, url: &str) -> bool;
}

/// Read-only registry interface for resolving platform adapters.
pub trait PlatformCatalog: Send + Sync {
    fn resolve_by_url(&self, url: &str) -> Option<Arc<dyn PlatformAdapter>>;
    fn resolve_by_kind(&self, kind: PlatformKind) -> Option<Arc<dyn PlatformAdapter>>;
}
