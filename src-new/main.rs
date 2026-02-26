use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use jessy_enrich::{EnrichRunInput, EnrichService};
use jessy_load::{LoadRunInput, LoadSeed, LoadService};
use jessy_prefilter::{PrefilterRunInput, PrefilterService};

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
    Prefilter {
        #[arg(long)]
        platform: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value = "manual_prefilter")]
        reason: String,
        #[arg(long = "avoid-word")]
        avoid_words_in_title: Vec<String>,
    },
    Enrich {
        #[arg(long)]
        platform: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value = "manual_enrich")]
        reason: String,
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

            let repo = outbound::sqlite_repo::SqliteRepo::new(cli.db_path);
            let service = LoadService::new(repo);
            let out = service.run(LoadRunInput { seeds, reason }).await?;
            println!("load processed={}", out.processed);
        }
        Command::Prefilter {
            platform,
            limit,
            reason,
            avoid_words_in_title,
        } => {
            let repo = outbound::sqlite_repo::SqliteRepo::new(cli.db_path);
            let service = PrefilterService::new(repo);
            let out = service
                .run(PrefilterRunInput {
                    platform_filter: platform,
                    limit,
                    reason,
                    avoid_words_in_title,
                })
                .await?;
            println!(
                "prefilter selected={} processed={} passed={} rejected={}",
                out.selected, out.processed, out.passed, out.rejected
            );
        }
        Command::Enrich {
            platform,
            limit,
            reason,
        } => {
            let repo = outbound::sqlite_repo::SqliteRepo::new(cli.db_path);
            let fetcher = outbound::enrich_agent::HttpUrlFetcher::new()?;
            let summarizer = outbound::enrich_agent::HeuristicSummaryGenerator::new();
            let service = EnrichService::new(repo, fetcher, summarizer);
            let out = service
                .run(EnrichRunInput {
                    platform_filter: platform,
                    limit,
                    reason,
                })
                .await?;
            println!(
                "enrich selected={} processed={} succeeded={} failed={}",
                out.selected, out.processed, out.succeeded, out.failed
            );
        }
    }

    Ok(())
}
