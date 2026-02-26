use anyhow::Result;
use clap::{Parser, Subcommand};
use jessy_extract::{ExtractRunInput, ExtractSeed, ExtractService};

mod extract_sqlite;

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
    Extract {
        #[arg(long = "url", required = true)]
        urls: Vec<String>,
        #[arg(long, required = true)]
        platform: String,
        #[arg(long, default_value = "manual_seed")]
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
        Command::Extract {
            urls,
            platform,
            reason,
            source_ref,
            source_cursor,
        } => {
            let seeds = urls
                .into_iter()
                .map(|canonical_url| ExtractSeed {
                    platform: platform.clone(),
                    canonical_url,
                    source_ref: source_ref.clone(),
                    source_cursor: source_cursor.clone(),
                })
                .collect::<Vec<_>>();

            let repo = extract_sqlite::SqliteExtractRepo::new(cli.db_path);
            let service = ExtractService::new(repo);
            let out = service.run(ExtractRunInput { seeds, reason }).await?;
            println!("extract processed={}", out.processed);
        }
    }

    Ok(())
}
