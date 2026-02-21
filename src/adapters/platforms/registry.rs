use std::sync::Arc;

use crate::domain::job::PlatformKind;
use crate::ports::platform::{PlatformAdapter, PlatformCatalog};

use super::linkedin::LinkedInAdapter;

/// In-memory registry of enabled platform adapters.
pub struct PlatformRegistry {
    adapters: Vec<Arc<dyn PlatformAdapter>>,
}

impl PlatformRegistry {
    /// Builds default registry for currently supported platforms.
    pub fn new_default() -> Self {
        Self {
            adapters: vec![Arc::new(LinkedInAdapter::new())],
        }
    }

    pub fn resolve_by_url(&self, url: &str) -> Option<Arc<dyn PlatformAdapter>> {
        self.adapters.iter().find(|p| p.matches_url(url)).cloned()
    }

    pub fn resolve_by_kind(&self, kind: PlatformKind) -> Option<Arc<dyn PlatformAdapter>> {
        self.adapters.iter().find(|p| p.kind() == kind).cloned()
    }
}

impl PlatformCatalog for PlatformRegistry {
    fn resolve_by_url(&self, url: &str) -> Option<Arc<dyn PlatformAdapter>> {
        self.resolve_by_url(url)
    }

    fn resolve_by_kind(&self, kind: PlatformKind) -> Option<Arc<dyn PlatformAdapter>> {
        self.resolve_by_kind(kind)
    }
}
