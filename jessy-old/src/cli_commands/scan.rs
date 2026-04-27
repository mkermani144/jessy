use anyhow::Result;

use crate::{
    adapters::{
        browser::chrome::ChromeBrowser, platforms::registry::PlatformRegistry,
        reporting::terminal::TerminalReporter, storage::sqlite::SqliteScanRepository,
    },
    config::AppConfig,
};

use super::{shared::build_ai_classifier, use_cases::scan};

pub async fn run(cfg: &AppConfig, browser: &ChromeBrowser, dry_run: bool) -> Result<()> {
    let storage = SqliteScanRepository::connect(&cfg.storage.db_path).await?;
    let ai = build_ai_classifier(cfg);
    let reporter = TerminalReporter;
    let platform_registry = PlatformRegistry::new_default();
    let deps = scan::ScanDeps {
        browser,
        storage: &storage,
        ai: ai.as_ref(),
        reporter: &reporter,
        platform_registry: &platform_registry,
    };

    scan::scan(cfg, &deps, dry_run).await
}
