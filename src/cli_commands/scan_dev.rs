use anyhow::Result;

use crate::{
    adapters::{
        browser::chrome::ChromeBrowser, platforms::registry::PlatformRegistry,
        reporting::terminal::TerminalReporter,
    },
    config::AppConfig,
};

use super::{shared::build_ai_classifier, use_cases::scan};

pub async fn run(cfg: &AppConfig, browser: &ChromeBrowser, dry_run: bool) -> Result<()> {
    let ai = build_ai_classifier(cfg);
    let reporter = TerminalReporter;
    let platform_registry = PlatformRegistry::new_default();
    let deps = scan::DevScanDeps {
        browser,
        ai: ai.as_ref(),
        reporter: &reporter,
        platform_registry: &platform_registry,
    };

    scan::scan_dev(cfg, &deps, dry_run).await
}
