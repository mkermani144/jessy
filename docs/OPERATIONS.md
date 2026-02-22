# Operations and Troubleshooting

## Commands

Use a config file explicitly when needed.

```bash
cargo run -- chrome-debug --config config/profile.yaml
cargo run -- doctor --config config/profile.yaml
cargo run -- scan --config config/profile.yaml
cargo run -- scan-dev --config config/profile.yaml
cargo run -- cleanup --config config/profile.yaml
cargo run -- cleanup --config config/profile.yaml --reset-history
```

## What each command does

- `chrome-debug`: ensure dedicated debug Chrome profile/session is available and open missing startup tabs from `chrome.startup_urls`.
- `doctor`: browser + SQLite + OpenAI health checks.
- `scan`: full job scan pipeline and report rendering.
- `scan-dev`: dev-only lightweight scan (ignores DB, scans current search page, prints reject reasons too).
- `cleanup`: retention cleanup only.
- `cleanup --reset-history`: clears full dedupe/history state.

## Config knobs that matter most

File: `config/profile.yaml`

- `chrome.debug_port`: CDP endpoint port.
- `chrome.profile_dir`: isolated profile directory.
- `chrome.startup_urls`: startup tabs for `chrome-debug` (opened only if missing).
- `sources.*`: currently parsed but not used by scan flow.
- `crawl.max_pages_per_search_tab`: max page depth per search tab.
- `crawl.stop_on_repeat_pages`: early stop on repeated fingerprint.
- `crawl.request_delay_ms`: currently reserved (not used in runtime flow).
- `filters.words_to_avoid_in_title`: cheap pre-open filter.
- `openai.model`: model used by Rig/OpenAI structured extractor.
- `retention.days`: automatic old-record cleanup window.
- `storage.db_path`: SQLite location.

## Logs: how to read quickly

Core run events:
- `chrome_connected`
- `run_start`
- `tab_discovery`
- `search_page_result` (`debug` level)
- `seed_done` or `seed_failed`
- `run_done`
- `openai_unload_not_required`

If `no_candidate_jobs_after_prefilter` appears:
- verify the debug profile actually has job/search tabs open.
- verify title prefilter (`filters.words_to_avoid_in_title`).
- verify history state (`cleanup --reset-history` if needed).

## Common issues

## No jobs appear

- No supported platform search tab is open in the debug Chrome profile.
- Scan may stop early on a search tab when it hits the first already-seen job.
- History may already contain current canonical URLs.
- Over-aggressive title avoid list can skip all seeds.

## "failed to connect SQLite"

- Ensure parent directory for `storage.db_path` is writable.
- Verify path is inside workspace or accessible from process.

## Repeated extraction failures

- Inspect current DOM shape and update platform selectors.
- Verify the required `aboutTheJob` selector still resolves after page load.

## OpenAI auth issues

- Configure one of:
  - `openai.api_key` (direct key in config), or
  - `openai.api_key_env` (environment variable name, default `OPENAI_API_KEY`).

## Debugging workflow

1. Run `doctor`.
2. Run `scan` with one known-good search tab open.
3. Inspect run-level events first.
4. If needed, refine selectors and rerun.
