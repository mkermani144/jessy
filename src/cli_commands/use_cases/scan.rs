use std::{sync::Arc, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use futures_util::stream::{self, StreamExt};
use sha2::{Digest, Sha256};
use tracing::{debug, info, info_span, warn, Instrument};

use crate::{
    config::AppConfig,
    domain::{
        ai::{AiInput, AiRequirements, CompanySize, EmploymentType, VisaPolicy, WorkMode},
        job::{JobRecord, JobStatus, PlatformKind, ReportRow},
        policy,
    },
    extract::job_page,
    ports::{
        ai::AiClassifier,
        browser::{BrowserAutomation, BrowserPageTab, BrowserSession, CandidateTab},
        platform::{PlatformAdapter, PlatformCatalog},
        reporting::RunReporter,
        storage::{RunCompletion, ScanRepository},
    },
};

/// Dependencies required by scan-related use cases.
///
/// This struct is the CLI composition boundary: use-cases depend only
/// on ports, while concrete adapters are wired by CLI/bootstrap code.
pub struct ScanDeps<'a> {
    /// Browser automation and session management.
    pub browser: &'a dyn BrowserAutomation,
    /// Persistence/repository operations.
    pub storage: &'a dyn ScanRepository,
    /// LLM classifier.
    pub ai: &'a dyn AiClassifier,
    /// Run output presenter.
    pub reporter: &'a dyn RunReporter,
    /// Platform adapter registry.
    pub platform_registry: &'a dyn PlatformCatalog,
}

/// Dependencies required by development-only scan mode.
///
/// This mode intentionally avoids any DB reads/writes and scans the current page.
pub struct DevScanDeps<'a> {
    /// Browser automation and session management.
    pub browser: &'a dyn BrowserAutomation,
    /// LLM classifier.
    pub ai: &'a dyn AiClassifier,
    /// Run output presenter.
    pub reporter: &'a dyn RunReporter,
    /// Platform adapter registry.
    pub platform_registry: &'a dyn PlatformCatalog,
}

const SELECTOR_RETRY_ATTEMPTS: usize = 3;
const SELECTOR_RETRY_DELAY_MS: u64 = 2200;
const DEFAULT_SEED_WORKERS: usize = 4;
const PAGE_READY_MAX_WAIT_MS: u64 = 5000;
const PAGE_READY_POLL_MS: u64 = 250;

#[derive(Debug, Clone)]
struct JobSeed {
    platform: PlatformKind,
    source_tab_url: String,
    page_index: i64,
    url: String,
    pre_title: Option<String>,
    pre_match_reason: String,
}

#[tracing::instrument(
    name = "scan",
    skip_all,
    fields(ai_model = %deps.ai.model_name())
)]
/// Main scan flow:
/// 1) discover candidate tabs
/// 2) extract jobs (search/detail)
/// 3) AI extraction + policy checks
/// 4) persist and render report
pub async fn scan(cfg: &AppConfig, deps: &ScanDeps<'_>) -> Result<()> {
    deps.browser.ensure_ready().await?;

    let version = deps.browser.version().await?;
    info!(
        event = "chrome_connected",
        browser = %version.browser,
        protocol = %version.protocol_version
    );

    deps.storage.cleanup_old_records(cfg.retention.days).await?;

    let run_id = deps.storage.start_run().await?;
    let started_at = Utc::now();
    info!(event = "run_start", run_id);

    let mut total_scanned = 0usize;
    let mut new_jobs = 0usize;
    let mut opportunities = 0usize;
    let mut not_opportunities = 0usize;

    let run_span = info_span!("run", run_id);
    let outcome = run_scan(
        cfg,
        deps,
        run_id,
        &mut total_scanned,
        &mut new_jobs,
        &mut opportunities,
        &mut not_opportunities,
    )
    .instrument(run_span)
    .await;

    let finished_at = Utc::now();

    match outcome {
        Ok(()) => {
            deps.storage
                .finish_run(&RunCompletion {
                    run_id,
                    status: "success".to_string(),
                    total_scanned,
                    new_jobs,
                    opportunities,
                    not_opportunities,
                    error: None,
                })
                .await?;

            let rows = deps.storage.load_report_rows(run_id).await?;
            deps.reporter.print_report(&rows, false);

            println!();
            println!("Run {} completed", run_id);
            println!("Started: {}", started_at);
            println!("Finished: {}", finished_at);
            println!("Scanned: {}", total_scanned);
            println!("New: {}", new_jobs);
            println!("Opportunity: {}", opportunities);
            println!("Not Opportunity: {}", not_opportunities);

            info!(
                event = "run_done",
                run_id,
                status = "success",
                scanned = total_scanned,
                new_jobs,
                opportunities,
                not_opportunities
            );

            if let Err(err) = deps.ai.unload_model().await {
                warn!(
                    event = "ai_unload_failed",
                    error = %sanitize_error_message(&err.to_string())
                );
            }
            Ok(())
        }
        Err(err) => {
            deps.storage
                .finish_run(&RunCompletion {
                    run_id,
                    status: "failed".to_string(),
                    total_scanned,
                    new_jobs,
                    opportunities,
                    not_opportunities,
                    error: Some(sanitize_error_message(&err.to_string())),
                })
                .await?;

            if let Err(unload_err) = deps.ai.unload_model().await {
                warn!(
                    event = "ai_unload_failed_after_error",
                    error = %sanitize_error_message(&unload_err.to_string())
                );
            }
            Err(err)
        }
    }
}

#[tracing::instrument(
    name = "scan_dev",
    skip_all,
    fields(ai_model = %deps.ai.model_name())
)]
/// Development scan mode:
/// - ignores database completely,
/// - scans jobs from the current search page,
/// - includes rejection reasons in output.
pub async fn scan_dev(cfg: &AppConfig, deps: &DevScanDeps<'_>) -> Result<()> {
    deps.browser.ensure_ready().await?;
    let version = deps.browser.version().await?;
    info!(
        event = "chrome_connected",
        browser = %version.browser,
        protocol = %version.protocol_version
    );

    let candidates = deps.browser.list_candidate_tabs().await?;
    info!(
        event = "dev_tab_discovery",
        candidate_tabs = candidates.len()
    );

    let Some((tab, adapter)) = find_first_search_tab(candidates, deps.platform_registry) else {
        println!("No matching search tab found for dev scan.");
        return Ok(());
    };

    let mut session = deps
        .browser
        .connect_session(&tab.websocket_debugger_url)
        .await
        .context("failed opening browser session for dev scan tab")?;

    let snapshot = extract_search_with_retry(session.as_mut(), adapter.as_ref())
        .await
        .context("search extraction failed in dev scan")?;

    let cards = if snapshot.job_cards.is_empty() {
        snapshot
            .job_links
            .iter()
            .map(|job_url| crate::domain::job::SearchCardData {
                title: String::new(),
                job_url: job_url.clone(),
            })
            .collect::<Vec<_>>()
    } else {
        snapshot.job_cards
    };

    let mut rows: Vec<(usize, ReportRow)> = Vec::new();
    let mut seeds: Vec<(usize, JobSeed)> = Vec::new();
    for (idx, card) in cards.into_iter().enumerate() {
        let pre_match = policy::title_pre_match(&cfg.filters, &card.title);
        if !pre_match.should_open_detail {
            rows.push((
                idx,
                ReportRow {
                    title: if card.title.trim().is_empty() {
                        "Unknown title".to_string()
                    } else {
                        card.title.clone()
                    },
                    company: None,
                    canonical_url: job_page::canonicalize_url(&card.job_url),
                    status: "not_opportunity".to_string(),
                    summary: format!("Rejected at prefilter: {}", pre_match.reason),
                    location: None,
                    work_mode: None,
                    employment_type: None,
                    posted_text: None,
                    compensation_text: None,
                    visa_policy_text: None,
                    description: None,
                    company_summary: None,
                    company_size: None,
                    requirements: vec![],
                },
            ));
            continue;
        }

        seeds.push((
            idx,
            JobSeed {
                platform: adapter.kind(),
                source_tab_url: tab.url.clone(),
                page_index: 1,
                url: card.job_url,
                pre_title: if card.title.trim().is_empty() {
                    None
                } else {
                    Some(card.title)
                },
                pre_match_reason: pre_match.reason,
            },
        ));
    }

    let worker_count = usize::min(DEFAULT_SEED_WORKERS, usize::max(1, seeds.len()));
    info!(
        event = "dev_seed_queue_ready",
        total = seeds.len(),
        workers = worker_count
    );

    let mut stream = stream::iter(seeds.into_iter().map(|(idx, seed)| {
        let fallback_title = seed
            .pre_title
            .clone()
            .unwrap_or_else(|| "Unknown title".to_string());
        let fallback_url = job_page::canonicalize_url(&seed.url);
        async move {
            let result = process_seed_dev(cfg, deps, seed).await;
            (idx, fallback_title, fallback_url, result)
        }
    }))
    .buffer_unordered(worker_count);

    while let Some((idx, fallback_title, fallback_url, result)) = stream.next().await {
        match result {
            Ok(row) => rows.push((idx, row)),
            Err(err) => {
                rows.push((
                    idx,
                    ReportRow {
                        title: fallback_title,
                        company: None,
                        canonical_url: fallback_url,
                        status: "not_opportunity".to_string(),
                        summary: format!(
                            "Rejected due to extraction/classification error: {}",
                            sanitize_error_message(&err.to_string())
                        ),
                        location: None,
                        work_mode: None,
                        employment_type: None,
                        posted_text: None,
                        compensation_text: None,
                        visa_policy_text: None,
                        description: None,
                        company_summary: None,
                        company_size: None,
                        requirements: vec![],
                    },
                ));
            }
        }
    }

    rows.sort_by_key(|(idx, _)| *idx);
    let rows = rows.into_iter().map(|(_, row)| row).collect::<Vec<_>>();

    deps.reporter.print_report(&rows, true);

    let opportunities = rows.iter().filter(|r| r.status == "opportunity").count();
    let rejected = rows.len().saturating_sub(opportunities);
    println!();
    println!("Dev scan completed");
    println!("Scanned from current page: {}", rows.len());
    println!("Opportunity: {}", opportunities);
    println!("Rejected: {}", rejected);

    if let Err(err) = deps.ai.unload_model().await {
        warn!(
            event = "ai_unload_failed",
            error = %sanitize_error_message(&err.to_string())
        );
    }

    Ok(())
}

#[tracing::instrument(name = "run_scan", skip_all, fields(run_id))]
async fn run_scan(
    cfg: &AppConfig,
    deps: &ScanDeps<'_>,
    run_id: i64,
    total_scanned: &mut usize,
    new_jobs: &mut usize,
    opportunities: &mut usize,
    not_opportunities: &mut usize,
) -> Result<()> {
    let all_tabs = deps.browser.list_tabs().await?;
    let candidates = deps.browser.list_candidate_tabs().await?;
    info!(
        event = "tab_discovery",
        total_tabs = all_tabs.len(),
        candidate_tabs = candidates.len()
    );

    if candidates.is_empty() {
        info!(event = "no_matching_tabs");
        return Ok(());
    }

    let mut seeds = Vec::new();
    for tab in candidates {
        let Some(adapter) = deps.platform_registry.resolve_by_url(&tab.url) else {
            info!(event = "tab_unsupported_platform");
            continue;
        };
        let platform = adapter.kind();
        let is_search = adapter.is_search_page(&tab.url);
        let mut session = deps
            .browser
            .connect_session(&tab.websocket_debugger_url)
            .await
            .context("failed opening browser session for candidate tab")?;

        if is_search {
            match scan_search_tab(cfg, deps, session.as_mut(), &tab, adapter.as_ref()).await {
                Ok(mut tab_seeds) => {
                    info!(
                        event = "tab_done",
                        tab_kind = "search",
                        platform = platform.as_str(),
                        seeds_added = tab_seeds.len()
                    );
                    seeds.append(&mut tab_seeds);
                }
                Err(err) => {
                    warn!(
                        event = "search_tab_failed",
                        platform = platform.as_str(),
                        error = %sanitize_error_message(&err.to_string())
                    );
                    continue;
                }
            }
        } else {
            seeds.push(JobSeed {
                platform,
                source_tab_url: tab.url.clone(),
                page_index: 1,
                url: tab.url.clone(),
                pre_title: None,
                pre_match_reason: "manual_detail_tab".to_string(),
            });
            info!(
                event = "tab_done",
                tab_kind = "detail",
                platform = platform.as_str(),
                seeds_added = 1
            );
        }
    }

    if seeds.is_empty() {
        info!(event = "no_candidate_jobs_after_prefilter");
        return Ok(());
    }

    let total_seeds = seeds.len();
    info!(event = "seed_queue_ready", count = total_seeds);

    let worker_count = usize::min(DEFAULT_SEED_WORKERS, usize::max(1, total_seeds));
    info!(event = "seed_workers", workers = worker_count);

    let mut stream = stream::iter(seeds.into_iter().enumerate().map(|(idx, seed)| async move {
        debug!(
            event = "seed_start",
            queue_index = idx + 1,
            queue_total = total_seeds,
            pre_match = %seed.pre_match_reason
        );
        let result = process_seed(cfg, deps, run_id, seed).await;
        (idx, result)
    }))
    .buffer_unordered(worker_count);

    while let Some((_idx, result)) = stream.next().await {
        *total_scanned += 1;
        match result {
            Ok((is_new, status)) => {
                if is_new {
                    *new_jobs += 1;
                }
                if status == "opportunity" {
                    *opportunities += 1;
                } else {
                    *not_opportunities += 1;
                }
                info!(event = "seed_done", status = %status, is_new);
            }
            Err(err) => {
                warn!(
                    event = "seed_failed",
                    error = %sanitize_error_message(&err.to_string())
                );
            }
        }
    }

    Ok(())
}

#[tracing::instrument(name = "scan_search_tab", skip_all)]
async fn scan_search_tab(
    cfg: &AppConfig,
    deps: &ScanDeps<'_>,
    session: &mut dyn BrowserSession,
    tab: &CandidateTab,
    adapter: &dyn PlatformAdapter,
) -> Result<Vec<JobSeed>> {
    let mut seen_links = std::collections::HashSet::new();
    let mut seen_fingerprints_in_run = std::collections::HashSet::new();
    let mut seeds = Vec::new();
    let tab_key = make_tab_key(&tab.url);

    for idx in 1..=cfg.crawl.max_pages_per_search_tab {
        let snapshot = extract_search_with_retry(session, adapter)
            .await
            .with_context(|| format!("search extraction failed on page {idx}"))?;

        let page_index = idx as i64;
        let fingerprint = hash_text(&snapshot.fingerprint_source);

        if cfg.crawl.stop_on_repeat_pages && !seen_fingerprints_in_run.insert(fingerprint.clone()) {
            info!(event = "stop_repeat_fingerprint_loop", page_index);
            break;
        }

        let seen_before = deps
            .storage
            .has_seen_page_fingerprint(&tab_key, &fingerprint)
            .await?;
        if seen_before {
            info!(event = "page_fingerprint_already_known", page_index);
        }

        deps.storage
            .record_page_fingerprint(&tab_key, &fingerprint, page_index)
            .await?;

        let cards = if snapshot.job_cards.is_empty() {
            snapshot
                .job_links
                .iter()
                .map(|job_url| crate::domain::job::SearchCardData {
                    title: String::new(),
                    job_url: job_url.clone(),
                })
                .collect::<Vec<_>>()
        } else {
            snapshot.job_cards.clone()
        };

        let total_cards = cards.len();
        let mut queued = 0usize;
        let mut skipped_seen = 0usize;
        let mut skipped_title = 0usize;
        let mut forced_open_no_title = 0usize;
        let mut stop_on_seen_job = false;

        for card in cards {
            let canonical = job_page::canonicalize_url(&card.job_url);
            if deps.storage.is_canonical_url_seen(&canonical).await? {
                skipped_seen += 1;
                stop_on_seen_job = true;
                info!(event = "stop_on_seen_job", page_index = idx);
                break;
            }
            if !seen_links.insert(canonical.clone()) {
                skipped_seen += 1;
                continue;
            }

            let decision = policy::title_pre_match(&cfg.filters, &card.title);
            if !decision.should_open_detail {
                skipped_title += 1;
                continue;
            }
            if card.title.trim().is_empty() {
                forced_open_no_title += 1;
            }

            seeds.push(JobSeed {
                platform: adapter.kind(),
                source_tab_url: tab.url.clone(),
                page_index,
                url: card.job_url,
                pre_title: if card.title.trim().is_empty() {
                    None
                } else {
                    Some(card.title)
                },
                pre_match_reason: decision.reason,
            });
            queued += 1;
        }

        debug!(
            event = "search_page_result",
            page_index = idx,
            cards = total_cards,
            queued,
            skipped_seen,
            skipped_title,
            forced_open_no_title,
            stop_on_seen_job
        );

        if stop_on_seen_job {
            break;
        }

        if idx >= cfg.crawl.max_pages_per_search_tab {
            break;
        }

        if let Some(next) = snapshot.next_page_url {
            info!(
                event = "search_page_advance",
                from_page = idx,
                to_page = idx + 1
            );
            session.navigate(&next).await?;
        } else {
            break;
        }
    }

    Ok(seeds)
}

#[tracing::instrument(
    name = "process_seed",
    skip_all,
    fields(source_page = seed.page_index, pre_match = %seed.pre_match_reason)
)]
async fn process_seed(
    cfg: &AppConfig,
    deps: &ScanDeps<'_>,
    run_id: i64,
    seed: JobSeed,
) -> Result<(bool, String)> {
    debug!(event = "open_detail_tab");

    let tab = deps
        .browser
        .open_tab(&seed.url)
        .await
        .context("failed opening detail tab")?;

    let result = process_opened_tab(cfg, deps, run_id, &seed, &tab).await;

    if let Err(err) = deps.browser.close_tab(&tab.id).await {
        warn!(
            event = "close_temp_tab_failed",
            tab_id = %tab.id,
            error = %sanitize_error_message(&err.to_string())
        );
    }

    result
}

fn find_first_search_tab(
    candidates: Vec<CandidateTab>,
    catalog: &dyn PlatformCatalog,
) -> Option<(CandidateTab, Arc<dyn PlatformAdapter>)> {
    for tab in candidates {
        let Some(adapter) = catalog.resolve_by_url(&tab.url) else {
            continue;
        };
        if adapter.is_search_page(&tab.url) {
            return Some((tab, adapter));
        }
    }
    None
}

#[tracing::instrument(
    name = "process_seed_dev",
    skip_all,
    fields(source_page = seed.page_index, pre_match = %seed.pre_match_reason)
)]
async fn process_seed_dev(
    cfg: &AppConfig,
    deps: &DevScanDeps<'_>,
    seed: JobSeed,
) -> Result<ReportRow> {
    let tab = deps
        .browser
        .open_tab(&seed.url)
        .await
        .context("failed opening detail tab")?;

    let result = process_opened_tab_dev(cfg, deps, &seed, &tab).await;

    if let Err(err) = deps.browser.close_tab(&tab.id).await {
        warn!(
            event = "close_temp_tab_failed",
            tab_id = %tab.id,
            error = %sanitize_error_message(&err.to_string())
        );
    }

    result
}

#[tracing::instrument(
    name = "process_detail",
    skip_all,
    fields(source_page = seed.page_index)
)]
async fn process_opened_tab(
    cfg: &AppConfig,
    deps: &ScanDeps<'_>,
    run_id: i64,
    seed: &JobSeed,
    tab: &BrowserPageTab,
) -> Result<(bool, String)> {
    let ws = tab
        .websocket_debugger_url
        .as_deref()
        .context("temporary tab missing websocket url")?;
    let mut session = deps.browser.connect_session(ws).await?;

    let adapter = deps
        .platform_registry
        .resolve_by_kind(seed.platform)
        .context("platform adapter not found for seed")?;
    let snapshot = extract_with_retry(session.as_mut(), adapter.as_ref()).await?;
    let dom_element = snapshot.about_job_dom.clone();
    let mut extraction = job_page::from_snapshot(snapshot);
    let raw_description = extraction.description.clone();
    let raw_requirements = extraction.requirements.clone();

    if deps
        .storage
        .is_canonical_url_seen(&extraction.canonical_url)
        .await?
    {
        debug!(event = "skip_already_seen");
        return Ok((false, "not_opportunity".to_string()));
    }

    if dom_element.trim().is_empty() {
        bail!("detail extraction missing aboutTheJob DOM element");
    }

    let ai_input = AiInput { dom_element };
    let ai_decision = deps
        .ai
        .classify(&ai_input)
        .await
        .context("ai extraction failed")?
        .sanitized();
    let work_mode_text = ai_decision.work_mode.as_ref().map(work_mode_to_string);
    let compensation_text = ai_decision.compensation_text.clone();
    let visa_policy_text = ai_decision
        .visa_policy_text
        .as_ref()
        .map(visa_policy_to_string);

    if let Some(title) = ai_decision.title.clone() {
        extraction.title = title;
    }
    if let Some(company_name) = ai_decision.company_name.clone() {
        extraction.company = company_name;
    }
    if let Some(location_text) = ai_decision.location_text.clone() {
        extraction.location = Some(location_text);
    }
    if let Some(employment_type) = ai_decision.employment_type.clone() {
        extraction.employment_type = Some(employment_type_to_string(&employment_type));
    }
    if let Some(description) = ai_decision.description.clone() {
        extraction.description = description;
    }
    let ai_requirements = flatten_requirements(&ai_decision.requirements);
    if !ai_requirements.is_empty() {
        extraction.requirements = ai_requirements;
    }
    if let Some(company_summary) = ai_decision.company_summary.clone() {
        extraction.company_summary = Some(company_summary);
    }
    if let Some(company_size_text) = ai_decision.company_size_text.clone() {
        extraction.company_size = Some(company_size_to_string(&company_size_text));
    }

    if extraction.title.trim().is_empty() {
        extraction.title = seed
            .pre_title
            .clone()
            .unwrap_or_else(|| infer_title_from_url(&extraction.canonical_url));
    }
    if extraction.company.trim().is_empty() {
        extraction.company = extraction
            .company_domain
            .as_deref()
            .map(domain_to_company_name)
            .unwrap_or_else(|| "Unknown Company".to_string());
    }
    if extraction.title.trim().is_empty() {
        bail!("extraction missing title after fallback");
    }

    let hard_reason = if matches!(
        ai_decision.visa_policy_text,
        Some(VisaPolicy::VisaNotSponsored)
    ) {
        Some("explicit_no_visa_or_sponsorship".to_string())
    } else {
        policy::hard_exclusion(&cfg.filters, &raw_description, &raw_requirements)
    };

    if let Some(hard_reason) = hard_reason {
        let dedupe_key = job_page::dedupe_key(
            &extraction.canonical_url,
            &extraction.company,
            &extraction.title,
        );
        let record = JobRecord {
            dedupe_key,
            canonical_url: extraction.canonical_url.clone(),
            company: extraction.company.clone(),
            title: extraction.title.clone(),
            location: extraction.location.clone(),
            work_mode: work_mode_text.clone(),
            employment_type: extraction.employment_type.clone(),
            posted_text: extraction.posted_text.clone(),
            compensation_text: compensation_text.clone(),
            visa_policy_text: visa_policy_text.clone(),
            description: extraction.description.clone(),
            requirements: extraction.requirements.clone(),
            source_tab_url: seed.source_tab_url.clone(),
            source_page_index: seed.page_index,
            status: JobStatus::NotOpportunity,
            status_reason: format!("Hard exclusion: {hard_reason}"),
            requirements_summary: "Hard exclusion rule matched".to_string(),
            company_summary: extraction
                .company_summary
                .clone()
                .unwrap_or_else(|| "No company summary extracted".to_string()),
            company_size: extraction.company_size.clone(),
        };

        let (_job_id, is_new) = deps.storage.upsert_job(run_id, &record).await?;
        info!(event = "hard_exclusion_applied", reason = %hard_reason, is_new);
        return Ok((is_new, "not_opportunity".to_string()));
    }

    let dedupe_key = job_page::dedupe_key(
        &extraction.canonical_url,
        &extraction.company,
        &extraction.title,
    );
    let status_reason = if seed.pre_match_reason.is_empty() {
        "Passed hard filters".to_string()
    } else {
        format!("Passed hard filters; pre_match={}", seed.pre_match_reason)
    };

    let requirements_summary = summarize_requirements(&extraction.requirements);
    let record = JobRecord {
        dedupe_key,
        canonical_url: extraction.canonical_url,
        company: extraction.company,
        title: extraction.title,
        location: extraction.location,
        work_mode: work_mode_text,
        employment_type: extraction.employment_type,
        posted_text: extraction.posted_text,
        compensation_text,
        visa_policy_text,
        description: extraction.description,
        requirements: extraction.requirements,
        source_tab_url: seed.source_tab_url.clone(),
        source_page_index: seed.page_index,
        status: JobStatus::Opportunity,
        status_reason,
        requirements_summary,
        company_summary: extraction
            .company_summary
            .clone()
            .unwrap_or_else(|| "No company summary extracted".to_string()),
        company_size: extraction.company_size.clone(),
    };

    let (_job_id, is_new) = deps.storage.upsert_job(run_id, &record).await?;
    debug!(event = "job_persisted", status = "opportunity", is_new);

    Ok((is_new, "opportunity".to_string()))
}

#[tracing::instrument(
    name = "process_detail_dev",
    skip_all,
    fields(source_page = seed.page_index)
)]
async fn process_opened_tab_dev(
    cfg: &AppConfig,
    deps: &DevScanDeps<'_>,
    seed: &JobSeed,
    tab: &BrowserPageTab,
) -> Result<ReportRow> {
    let ws = tab
        .websocket_debugger_url
        .as_deref()
        .context("temporary tab missing websocket url")?;
    let mut session = deps.browser.connect_session(ws).await?;

    let adapter = deps
        .platform_registry
        .resolve_by_kind(seed.platform)
        .context("platform adapter not found for seed")?;
    let snapshot = extract_with_retry(session.as_mut(), adapter.as_ref()).await?;
    let dom_element = snapshot.about_job_dom.clone();
    let mut extraction = job_page::from_snapshot(snapshot);
    let raw_description = extraction.description.clone();
    let raw_requirements = extraction.requirements.clone();

    if dom_element.trim().is_empty() {
        bail!("detail extraction missing aboutTheJob DOM element");
    }

    let ai_input = AiInput { dom_element };
    let ai_decision = deps
        .ai
        .classify(&ai_input)
        .await
        .context("ai extraction failed")?
        .sanitized();
    let work_mode_text = ai_decision.work_mode.as_ref().map(work_mode_to_string);
    let compensation_text = ai_decision.compensation_text.clone();
    let visa_policy_text = ai_decision
        .visa_policy_text
        .as_ref()
        .map(visa_policy_to_string);

    if let Some(title) = ai_decision.title.clone() {
        extraction.title = title;
    }
    if let Some(company_name) = ai_decision.company_name.clone() {
        extraction.company = company_name;
    }
    if let Some(location_text) = ai_decision.location_text.clone() {
        extraction.location = Some(location_text);
    }
    if let Some(employment_type) = ai_decision.employment_type.clone() {
        extraction.employment_type = Some(employment_type_to_string(&employment_type));
    }
    if let Some(description) = ai_decision.description.clone() {
        extraction.description = description;
    }
    let ai_requirements = flatten_requirements(&ai_decision.requirements);
    if !ai_requirements.is_empty() {
        extraction.requirements = ai_requirements;
    }
    if let Some(company_summary) = ai_decision.company_summary.clone() {
        extraction.company_summary = Some(company_summary);
    }
    if let Some(company_size_text) = ai_decision.company_size_text.clone() {
        extraction.company_size = Some(company_size_to_string(&company_size_text));
    }

    if extraction.title.trim().is_empty() {
        extraction.title = seed
            .pre_title
            .clone()
            .unwrap_or_else(|| infer_title_from_url(&extraction.canonical_url));
    }
    if extraction.company.trim().is_empty() {
        extraction.company = extraction
            .company_domain
            .as_deref()
            .map(domain_to_company_name)
            .unwrap_or_else(|| "Unknown Company".to_string());
    }
    if extraction.title.trim().is_empty() {
        bail!("extraction missing title after fallback");
    }

    let hard_reason = if matches!(
        ai_decision.visa_policy_text,
        Some(VisaPolicy::VisaNotSponsored)
    ) {
        Some("explicit_no_visa_or_sponsorship".to_string())
    } else {
        policy::hard_exclusion(&cfg.filters, &raw_description, &raw_requirements)
    };

    if let Some(hard_reason) = hard_reason {
        return Ok(ReportRow {
            title: extraction.title,
            company: Some(extraction.company),
            canonical_url: extraction.canonical_url,
            status: "not_opportunity".to_string(),
            summary: format!("Rejected: hard exclusion ({hard_reason})"),
            location: extraction.location,
            work_mode: work_mode_text,
            employment_type: extraction.employment_type,
            posted_text: extraction.posted_text,
            compensation_text,
            visa_policy_text,
            description: Some(extraction.description),
            company_summary: extraction.company_summary,
            company_size: extraction.company_size,
            requirements: extraction.requirements,
        });
    }

    let summary = if seed.pre_match_reason.is_empty() {
        "Passed hard filters".to_string()
    } else {
        format!("Passed hard filters; pre_match={}", seed.pre_match_reason)
    };

    Ok(ReportRow {
        title: extraction.title,
        company: Some(extraction.company),
        canonical_url: extraction.canonical_url,
        status: "opportunity".to_string(),
        summary,
        location: extraction.location,
        work_mode: work_mode_text,
        employment_type: extraction.employment_type,
        posted_text: extraction.posted_text,
        compensation_text,
        visa_policy_text,
        description: Some(extraction.description),
        company_summary: extraction.company_summary,
        company_size: extraction.company_size,
        requirements: extraction.requirements,
    })
}

/// Diagnostic checks for browser, storage, and AI provider health.
pub async fn doctor(cfg: &AppConfig, deps: &ScanDeps<'_>) -> Result<()> {
    println!("Jessy doctor checks");

    deps.browser.ensure_ready().await?;
    let version = deps.browser.version().await?;
    let tabs = deps.browser.list_tabs().await?;

    println!("- Chrome version: {}", version.browser);
    println!("- CDP protocol: {}", version.protocol_version);
    println!("- Open page tabs: {}", tabs.len());

    deps.storage.healthcheck().await?;
    println!("- SQLite: OK ({})", cfg.storage.db_path);

    match deps.ai.healthcheck().await {
        Ok(_) => println!("- AI provider: OK ({})", deps.ai.model_name()),
        Err(err) => println!(
            "- AI provider: WARNING ({})",
            sanitize_error_message(&err.to_string())
        ),
    }

    println!("Doctor completed.");
    Ok(())
}

/// Cleanup command implementation.
///
/// When `reset_history` is true, all dedupe/history state is cleared.
pub async fn cleanup(
    cfg: &AppConfig,
    storage: &dyn ScanRepository,
    reset_history: bool,
) -> Result<()> {
    if reset_history {
        storage.clear_all_history().await?;
        println!("Cleanup completed. Full history reset done.");
        return Ok(());
    }

    let deleted = storage.cleanup_old_records(cfg.retention.days).await?;
    println!(
        "Cleanup completed. Deleted old jobs: {deleted} (retention.days={})",
        cfg.retention.days
    );
    println!("Tip: use `jessy cleanup --reset-history` to clear all dedupe history.");
    Ok(())
}

fn make_tab_key(url: &str) -> String {
    hash_text(url)
}

fn hash_text(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn summarize_requirements(reqs: &[String]) -> String {
    if reqs.is_empty() {
        return "No explicit requirements extracted".to_string();
    }

    reqs.iter().take(5).cloned().collect::<Vec<_>>().join("; ")
}

fn flatten_requirements(reqs: &AiRequirements) -> Vec<String> {
    let mut out = Vec::new();
    for item in reqs
        .languages
        .iter()
        .chain(reqs.frameworks.iter())
        .chain(reqs.tools.iter())
        .chain(reqs.databases.iter())
        .chain(reqs.cloud.iter())
        .chain(reqs.other.iter())
    {
        let clean = item.trim();
        if clean.is_empty() {
            continue;
        }
        if !out.iter().any(|v: &String| v.eq_ignore_ascii_case(clean)) {
            out.push(clean.to_string());
        }
    }
    out
}

fn employment_type_to_string(value: &EmploymentType) -> String {
    match value {
        EmploymentType::FullTime => "full_time".to_string(),
        EmploymentType::PartTime => "part_time".to_string(),
        EmploymentType::Contract => "contract".to_string(),
        EmploymentType::Internship => "internship".to_string(),
        EmploymentType::Temporary => "temporary".to_string(),
        EmploymentType::Freelance => "freelance".to_string(),
    }
}

fn work_mode_to_string(value: &WorkMode) -> String {
    match value {
        WorkMode::Remote => "remote".to_string(),
        WorkMode::Hybrid => "hybrid".to_string(),
        WorkMode::OnSite => "on_site".to_string(),
    }
}

fn visa_policy_to_string(value: &VisaPolicy) -> String {
    match value {
        VisaPolicy::VisaSponsored => "visa_sponsored".to_string(),
        VisaPolicy::Unknown => "unknown".to_string(),
        VisaPolicy::VisaNotSponsored => "visa_not_sponsored".to_string(),
    }
}

fn company_size_to_string(value: &CompanySize) -> String {
    match value {
        CompanySize::OneToTen => "1-10".to_string(),
        CompanySize::ElevenToFifty => "11-50".to_string(),
        CompanySize::FiftyOneToFiveHundred => "51-500".to_string(),
        CompanySize::Above => "above".to_string(),
    }
}

#[tracing::instrument(name = "extract_with_retry", skip_all)]
async fn extract_with_retry(
    session: &mut dyn BrowserSession,
    adapter: &dyn PlatformAdapter,
) -> Result<crate::domain::job::JobDetailData> {
    wait_for_page_ready(session, PAGE_READY_MAX_WAIT_MS).await;

    let mut last_partial = None;
    let mut last_err = None;

    for attempt in 0..SELECTOR_RETRY_ATTEMPTS {
        match adapter.extract_job_detail(session).await {
            Ok(snapshot) => {
                if is_detail_snapshot_usable(&snapshot) {
                    debug!(
                        event = "extract_attempt",
                        attempt = attempt + 1,
                        status = "usable"
                    );
                    return Ok(snapshot);
                }
                let title_present = !snapshot.title.trim().is_empty();
                let desc_len = snapshot.description.len();
                last_partial = Some(snapshot);
                debug!(
                    event = "extract_attempt",
                    attempt = attempt + 1,
                    status = "selector_missing_retry",
                    title_present,
                    desc_len
                );
            }
            Err(err) => {
                let err_msg = sanitize_error_message(&err.to_string());
                last_err = Some(err);
                debug!(
                    event = "extract_attempt",
                    attempt = attempt + 1,
                    status = "error_retry",
                    error = %err_msg
                );
            }
        }
        if attempt + 1 < SELECTOR_RETRY_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(SELECTOR_RETRY_DELAY_MS)).await;
            wait_for_page_ready(session, PAGE_READY_MAX_WAIT_MS).await;
        }
    }

    if let Some(snapshot) = last_partial {
        let title_present = !snapshot.title.trim().is_empty();
        let desc_len = snapshot.description.len();
        warn!(
            event = "detail_selector_missing_after_retry",
            attempts = SELECTOR_RETRY_ATTEMPTS,
            title_present,
            desc_len
        );
        return Err(anyhow!(
            "detail extraction missing required selectors after retry"
        ));
    }

    if let Some(err) = last_err {
        warn!(
            event = "detail_extraction_failed_after_retry",
            attempts = SELECTOR_RETRY_ATTEMPTS,
            error = %sanitize_error_message(&err.to_string())
        );
        return Err(err).context("failed extracting detail");
    }

    bail!("failed extracting detail")
}

#[tracing::instrument(name = "extract_search_with_retry", skip_all)]
async fn extract_search_with_retry(
    session: &mut dyn BrowserSession,
    adapter: &dyn PlatformAdapter,
) -> Result<crate::domain::job::SearchPageData> {
    wait_for_page_ready(session, PAGE_READY_MAX_WAIT_MS).await;

    let mut last_partial = None;
    let mut last_err = None;

    for attempt in 0..SELECTOR_RETRY_ATTEMPTS {
        match adapter.extract_search(session).await {
            Ok(snapshot) => {
                if is_search_snapshot_usable(&snapshot) {
                    debug!(
                        event = "search_extract_attempt",
                        attempt = attempt + 1,
                        status = "usable",
                        cards = snapshot.job_cards.len(),
                        links = snapshot.job_links.len()
                    );
                    return Ok(snapshot);
                }

                let cards = snapshot.job_cards.len();
                let links = snapshot.job_links.len();
                last_partial = Some(snapshot);
                debug!(
                    event = "search_extract_attempt",
                    attempt = attempt + 1,
                    status = "selector_missing_retry",
                    cards,
                    links
                );
            }
            Err(err) => {
                let err_msg = sanitize_error_message(&err.to_string());
                last_err = Some(err);
                debug!(
                    event = "search_extract_attempt",
                    attempt = attempt + 1,
                    status = "error_retry",
                    error = %err_msg
                );
            }
        }

        if attempt + 1 < SELECTOR_RETRY_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(SELECTOR_RETRY_DELAY_MS)).await;
            wait_for_page_ready(session, PAGE_READY_MAX_WAIT_MS).await;
        }
    }

    if let Some(snapshot) = last_partial {
        warn!(
            event = "search_selector_missing_after_retry",
            attempts = SELECTOR_RETRY_ATTEMPTS,
            cards = snapshot.job_cards.len(),
            links = snapshot.job_links.len()
        );
        return Err(anyhow!(
            "search extraction missing required selectors after retry"
        ));
    }

    if let Some(err) = last_err {
        warn!(
            event = "search_extraction_failed_after_retry",
            attempts = SELECTOR_RETRY_ATTEMPTS,
            error = %sanitize_error_message(&err.to_string())
        );
        return Err(err).context("failed extracting search page");
    }

    bail!("failed extracting search page")
}

fn is_search_snapshot_usable(snapshot: &crate::domain::job::SearchPageData) -> bool {
    !(snapshot.job_cards.is_empty() && snapshot.job_links.is_empty())
}

fn is_detail_snapshot_usable(snapshot: &crate::domain::job::JobDetailData) -> bool {
    let about_job_dom_present = !snapshot.about_job_dom.trim().is_empty();
    let title_present = !snapshot.title.trim().is_empty();
    let description_present = snapshot.description.trim().len() > 20;
    about_job_dom_present && (title_present || description_present)
}

fn domain_to_company_name(domain: &str) -> String {
    let core = domain
        .trim()
        .trim_start_matches("www.")
        .split('.')
        .next()
        .unwrap_or("Unknown");

    let words = core
        .split(['-', '_'])
        .filter(|x| !x.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();

    if words.is_empty() {
        "Unknown Company".to_string()
    } else {
        words.join(" ")
    }
}

fn infer_title_from_url(url: &str) -> String {
    let lowered = url.to_ascii_lowercase();
    if lowered.contains("linkedin.com") {
        return "LinkedIn Job Posting".to_string();
    }
    if lowered.contains("indeed.com") {
        return "Indeed Job Posting".to_string();
    }
    "Job Posting".to_string()
}

async fn wait_for_page_ready(session: &mut dyn BrowserSession, max_wait_ms: u64) {
    let mut elapsed = 0u64;
    while elapsed < max_wait_ms {
        let ready = session
            .evaluate("(() => document.readyState || '')()")
            .await
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        if ready == "interactive" || ready == "complete" {
            return;
        }

        tokio::time::sleep(Duration::from_millis(PAGE_READY_POLL_MS)).await;
        elapsed += PAGE_READY_POLL_MS;
    }
}

fn sanitize_error_message(raw: &str) -> String {
    let mut out = raw.to_string();

    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            out = out.replace(trimmed, "[REDACTED]");
        }
    }

    if let Some(start) = out.find("sk-") {
        let tail = &out[start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ')')
            .unwrap_or(tail.len());
        let secret = &tail[..end];
        if !secret.is_empty() {
            out = out.replace(secret, "[REDACTED]");
        }
    }

    out
}
