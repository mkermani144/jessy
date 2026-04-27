# Scan Procedure

This document describes what `jessy scan` does from start to finish.

## High-level flow

1. Ensure dedicated Chrome debug profile is available.
2. Initialize storage and retention cleanup.
3. Start a run log in SQLite.
4. Discover candidate tabs from open Chrome tabs.
5. For each candidate search tab, extract job cards page-by-page.
6. Pre-filter by title before opening detail tabs.
7. Open temporary detail tabs only for candidates.
8. Extract job detail snapshot and `aboutTheJob` DOM.
9. Run AI structured extraction (single-shot, per job DOM).
10. Apply hard exclusions using raw text plus AI visa signal.
11. Persist status/results/observations.
12. Render terminal cards.
13. Finish run log and run provider unload/teardown hook.

## Detailed steps

## 1) Browser startup

- Use `BrowserAutomation::ensure_ready()`.
- If `chrome.auto_launch: true`, Jessy launches Chrome with remote-debugging enabled.
- Uses dedicated `chrome.profile_dir` for isolation.

## 2) Storage setup

- `SqliteScanRepository::connect` opens DB and applies migration.
- Retention is applied before scan via `cleanup_old_records(retention.days)`.

## 3) Run bookkeeping

- `start_run()` creates a running run row.
- Final counters are updated through `finish_run()`.

## 4) Tab discovery

- Fetch all tabs with `BrowserAutomation::list_tabs()`.
- Candidate list comes from all tabs in the dedicated debug profile (no source filtering).

## 5) Search-page extraction loop

For each search tab:
- Connect a browser session to tab websocket.
- Call platform adapter `extract_search()`.
- Platform-specific selectors/scripts are owned by the platform adapter package (not `src/chrome/*`).
- Save page fingerprint per tab key.
- If repeat fingerprint and `crawl.stop_on_repeat_pages: true`, stop paging.
- Advance page via extracted next-page URL up to `crawl.max_pages_per_search_tab`.

## 6) Seed generation and prefilter

- Build `JobSeed` from extracted cards.
- Stop scanning this search tab at the first canonical URL already seen in DB (assumes newest-first ordering).
- Apply title prefilter (`domain::policy::title_pre_match`).
- Only remaining seeds become detail-page candidates.

## 7) Detail extraction

For each seed:
- Open temporary tab for job URL.
- Connect session.
- Call platform adapter `extract_job_detail()` with retry.
- Extraction/parsing details stay inside platform-owned extractors.
- Normalize through `extract::job_page::from_snapshot`.
- Keep the search-card title as authoritative when present.
- Require non-empty `about_job_dom` (retry on selector delays).
- Fill fallback title/company if missing.
- Close temporary tab.

## 8) AI structured extraction

- Build `AiInput` with one field: the extracted `about_job_dom`.
- Call `AiClassifier::classify()` (Rig provider adapter).
- AI returns structured fields only (company/location/requirements/etc); title is not AI-owned.
- Each job is extracted independently (no shared conversation state).

## 9) Hard exclusions

- Run `policy::hard_exclusion()` on raw extracted description/requirements.
- Also treat AI `visa_policy_text = visa_not_sponsored` as hard exclusion.
- Hard exclusion classes:
  - explicit no-visa/no-sponsorship requirements.
  - explicit country residency/location-only requirements.
- If hard-excluded, persist as `not_opportunity` immediately.

## 10) Persistence

- Build `JobRecord`.
- `upsert_job()` updates/creates job row and run linkage.
- Status assignment in current flow:
  - `not_opportunity` if hard exclusion matched.
  - `opportunity` otherwise.
- Also writes:
  - run-job result,
  - job observation.

## 11) Reporting and teardown

- Load run report rows.
- Render with `RunReporter` (terminal cards).
- Finish run status/counters.
- Call `AiClassifier::unload_model()` (provider-specific behavior).

## Timing controls

- Search-page limit: `crawl.max_pages_per_search_tab`.
- Repeat stop: `crawl.stop_on_repeat_pages`.
- `crawl.request_delay_ms` exists in config but is currently reserved (not used in runtime flow).
