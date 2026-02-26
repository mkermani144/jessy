use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use jessy_enrich::{EnrichRunInput, EnrichService};
use jessy_extract::{ExtractRunInput, ExtractService, ExtractSource};
use jessy_load::{LoadRunInput, LoadSeed, LoadService};
use jessy_prefilter::{PrefilterRunInput, PrefilterService};
use jessy_serve::{ServeRunInput, ServeService};

mod inbound;
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
    Extract {
        #[arg(long, required = true)]
        platform: String,
        #[arg(long = "source-ref", required = true)]
        source_refs: Vec<String>,
        #[arg(long)]
        source_cursor: Option<String>,
        #[arg(long, default_value_t = 1)]
        max_pages_per_source: usize,
        #[arg(long, default_value_t = 50)]
        max_links_per_page: usize,
        #[arg(long, default_value = "manual_extract")]
        reason: String,
    },
    Load {
        #[arg(long = "url")]
        urls: Vec<String>,
        #[arg(long)]
        platform: Option<String>,
        #[arg(long, default_value = "manual_load")]
        reason: String,
        #[arg(long, default_value = "manual://input")]
        source_ref: String,
        #[arg(long)]
        source_cursor: Option<String>,
        #[arg(long, default_value_t = 100)]
        pending_limit: usize,
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
    Serve {
        #[arg(long)]
        platform: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        query: Option<String>,
        #[arg(long, default_value_t = false)]
        no_fzf: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Extract {
            platform,
            source_refs,
            source_cursor,
            max_pages_per_source,
            max_links_per_page,
            reason,
        } => {
            let sources = source_refs
                .into_iter()
                .map(|source_ref| ExtractSource {
                    platform: platform.clone(),
                    source_ref,
                    source_cursor: source_cursor.clone(),
                })
                .collect::<Vec<_>>();

            let repo = outbound::sqlite_repo::SqliteRepo::new(cli.db_path);
            let crawler = outbound::extract_crawler::HttpExtractCrawler::new()?;
            let service = ExtractService::new(repo, crawler);
            let out = service
                .run(ExtractRunInput {
                    sources,
                    max_pages_per_source,
                    max_links_per_page,
                    reason,
                })
                .await?;
            println!(
                "extract sources={} pages={} discovered={} emitted={}",
                out.selected_sources, out.crawled_pages, out.discovered, out.emitted
            );
        }
        Command::Load {
            urls,
            platform,
            reason,
            source_ref,
            source_cursor,
            pending_limit,
        } => {
            if !urls.is_empty() && platform.is_none() {
                bail!("load --platform is required when --url is provided");
            }
            let seeds = urls
                .into_iter()
                .map(|canonical_url| LoadSeed {
                    platform: platform.clone().unwrap_or_default(),
                    canonical_url,
                    source_ref: source_ref.clone(),
                    source_cursor: source_cursor.clone(),
                })
                .collect::<Vec<_>>();

            let repo = outbound::sqlite_repo::SqliteRepo::new(cli.db_path);
            let service = LoadService::new(repo);
            let out = service
                .run(LoadRunInput {
                    seeds,
                    reason,
                    platform_filter: platform,
                    pending_limit,
                })
                .await?;
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
        Command::Serve {
            platform,
            limit,
            query,
            no_fzf,
        } => {
            let repo = outbound::sqlite_repo::SqliteRepo::new(cli.db_path);
            let channel = inbound::serve_terminal::TerminalChannel::new(!no_fzf);
            let service = ServeService::new(repo, channel);
            service
                .run(ServeRunInput {
                    platform_filter: platform,
                    limit,
                    query,
                })
                .await?;
        }
    }

    Ok(())
}
