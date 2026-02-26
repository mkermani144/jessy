use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use jessy_load::{LoadRunInput, LoadSeed, LoadService};

mod outbound;

#[derive(Debug, Parser)]
#[command(name = "jessy-new", version, about = "Jessy pipeline CLI (new wiring)")]
struct Cli {
    #[arg(long, global = true, default_value = "data/jessy.db")]
    db_path: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Extract,
    Load {
        #[arg(long = "url", required = true)]
        urls: Vec<String>,
        #[arg(long, required = true)]
        platform: String,
        #[arg(long, default_value = "manual_load")]
        reason: String,
        #[arg(long, default_value = "manual://input")]
        source_ref: String,
        #[arg(long)]
        source_cursor: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Extract => {
            bail!("extract not implemented yet (crawl-only step planned); use `load` for now");
        }
        Command::Load {
            urls,
            platform,
            reason,
            source_ref,
            source_cursor,
        } => {
            let seeds = urls
                .into_iter()
                .map(|canonical_url| LoadSeed {
                    platform: platform.clone(),
                    canonical_url,
                    source_ref: source_ref.clone(),
                    source_cursor: source_cursor.clone(),
                })
                .collect::<Vec<_>>();

            let repo = outbound::load_sqlite::SqliteLoadRepo::new(cli.db_path);
            let service = LoadService::new(repo);
            let out = service.run(LoadRunInput { seeds, reason }).await?;
            println!("load processed={}", out.processed);
        }
    }

    Ok(())
}
