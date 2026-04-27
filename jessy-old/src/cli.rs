use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

use crate::{adapters::browser::chrome::ChromeBrowser, cli_commands, config::AppConfig};

#[derive(Debug, Parser)]
#[command(
    name = "jessy",
    version,
    about = "Read-only job intelligence assistant"
)]
struct Cli {
    #[arg(short, long, global = true, default_value = "config/profile.yaml")]
    config: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Scan {
        /// Skip AI classification calls; extraction + deterministic filters only.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    #[command(name = "scan-dev")]
    ScanDev {
        /// Skip AI classification calls; extraction + deterministic filters only.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    Doctor,
    #[command(name = "chrome-debug", alias = "chrome")]
    ChromeDebug,
    Cleanup {
        /// Reset all scan history/dedupe state (jobs, runs, fingerprints)
        #[arg(long, default_value_t = false)]
        reset_history: bool,
    },
}

pub async fn run() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let cfg = AppConfig::load(&cli.config)?;
    let browser = ChromeBrowser::new(cfg.chrome.clone());

    match cli.command {
        Command::Scan { dry_run } => cli_commands::scan::run(&cfg, &browser, dry_run).await,
        Command::ScanDev { dry_run } => cli_commands::scan_dev::run(&cfg, &browser, dry_run).await,
        Command::Doctor => cli_commands::doctor::run(&cfg, &browser).await,
        Command::ChromeDebug => cli_commands::chrome_debug::run(&cfg, &browser).await,
        Command::Cleanup { reset_history } => cli_commands::cleanup::run(&cfg, reset_history).await,
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init();
}
