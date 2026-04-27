use std::collections::HashSet;

use anyhow::Result;
use tracing::{info, warn};

use crate::{config::AppConfig, ports::browser::BrowserAutomation};

/// Ensures dedicated debug Chrome profile is available and prints diagnostics.
pub async fn open_debug_chrome(cfg: &AppConfig, browser: &dyn BrowserAutomation) -> Result<()> {
    let endpoint = browser.debug_endpoint();
    let was_running = browser.version().await.is_ok();

    browser.ensure_ready().await?;
    let version = browser.version().await?;
    let tabs = browser.list_tabs().await?;
    let mut existing_urls: HashSet<String> = tabs
        .iter()
        .map(|t| t.url.to_ascii_lowercase())
        .collect::<HashSet<_>>();

    let mut opened = 0usize;
    let mut skipped_existing = 0usize;
    for startup_url in &cfg.chrome.startup_urls {
        let key = startup_url.to_ascii_lowercase();
        let already_open = existing_urls.iter().any(|u| u.starts_with(&key));
        if already_open {
            skipped_existing += 1;
            continue;
        }

        match browser.open_tab(startup_url).await {
            Ok(tab) => {
                existing_urls.insert(tab.url.to_ascii_lowercase());
                opened += 1;
            }
            Err(err) => {
                warn!(
                    event = "chrome_startup_tab_open_failed",
                    url = %startup_url,
                    error = %err
                );
            }
        }
    }

    info!(
        event = "chrome_debug_ready",
        status = if was_running {
            "already_running"
        } else {
            "launched"
        },
        browser = %version.browser
    );

    println!("Chrome debug profile ready.");
    println!(
        "Status: {}",
        if was_running {
            "already running"
        } else {
            "launched now"
        }
    );
    println!("Browser: {}", version.browser);
    println!("Endpoint: {endpoint}");
    println!("Profile dir: {}", browser.profile_dir());
    println!("Startup tabs opened: {opened}");
    println!("Startup tabs already open: {skipped_existing}");

    Ok(())
}
