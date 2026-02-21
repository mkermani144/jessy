use anyhow::Result;

use crate::{adapters::storage::sqlite::SqliteScanRepository, config::AppConfig};

use super::use_cases::scan;

pub async fn run(cfg: &AppConfig, reset_history: bool) -> Result<()> {
    let storage = SqliteScanRepository::connect(&cfg.storage.db_path).await?;
    scan::cleanup(cfg, &storage, reset_history).await
}
