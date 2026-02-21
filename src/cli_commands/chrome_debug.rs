use anyhow::Result;

use crate::{adapters::browser::chrome::ChromeBrowser, config::AppConfig};

use super::use_cases::chrome;

pub async fn run(cfg: &AppConfig, browser: &ChromeBrowser) -> Result<()> {
    chrome::open_debug_chrome(cfg, browser).await
}
