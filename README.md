# Jessy

Jessy is a local, read-only job search assistant for open Chrome tabs.

It uses:
- Rust for orchestration,
- Chrome DevTools Protocol (non-headless) for tab crawling,
- Rig + OpenAI for structured extraction from job-detail DOM,
- SQLite for history and dedupe.

## What it does

- Reads open tabs from the dedicated debug Chrome profile (assumed job-related).
- Crawls search pages up to N pages with repeat-page stop.
- Runs title prefilter with `filters.words_to_avoid_in_title` before opening detail tabs.
- Extracts one `aboutTheJob` DOM element per candidate and asks AI for structured fields (including language).
- Applies deterministic hard exclusions (visa/residency); hard-excluded jobs become `not_opportunity`, others become `opportunity`.
- Stores history in SQLite to avoid repeated work.
- Prints verbose terminal cards.

## Safety mode

Jessy is read-only and safe-mode by design:
- no auto-apply,
- no write actions on job platforms,
- no automated LinkedIn people-tab crawling.

## Setup

1. Copy config:

```bash
cp config/profile.example.yaml config/profile.yaml
```

2. Configure your OpenAI API key (either in config or via env var):

```bash
export OPENAI_API_KEY=...
```

Or set it directly in `config/profile.yaml`:

```yaml
openai:
  api_key: sk-...
```

3. Define startup URLs to open with `chrome-debug` in `config/profile.yaml`:

```yaml
chrome:
  startup_urls:
    - https://www.linkedin.com/jobs/search/
    - https://web.telegram.org/
```

Tabs for unsupported platforms can still be opened here; scanner logic will ignore tabs that do not match a registered platform adapter.

4. Run checks:

```bash
cargo run -- doctor --config config/profile.yaml
```

5. Launch dedicated Chrome debug profile (optional but useful):

```bash
cargo run -- chrome-debug --config config/profile.yaml
```

`chrome-debug` also opens missing startup tabs from `chrome.startup_urls`.

6. Scan:

```bash
cargo run -- scan --config config/profile.yaml
```

Dev-only sample scan (no DB, whole current search page):

```bash
cargo run -- scan-dev --config config/profile.yaml
```

7. Cleanup old records:

```bash
cargo run -- cleanup --config config/profile.yaml
```

## Chrome notes

Jessy expects Chrome DevTools endpoint on `debug_port` (default `9222`).
If `auto_launch: true`, Jessy launches Chrome with:
- `--remote-debugging-port=<port>`
- `--user-data-dir=<profile_dir>`

Use a dedicated profile directory for isolation.
`scan` now assumes this dedicated debug profile contains job-related tabs and does not apply source filtering.

## Data

- SQLite DB path: `storage.db_path` (default `data/jessy.db`)
- Initial schema: `migrations/001_init.sql`

## Current limitations

- LinkedIn DOM can change; selectors may need updates.
- Only LinkedIn has a platform adapter right now.

## Documentation

- `docs/README.md`: docs index.
- `docs/PROCEDURE.md`: end-to-end scan procedure.
- `docs/ARCHITECTURE.md`: hexarch boundaries + important files.
- `docs/ADDING_PLATFORM.md`: how to add a new platform adapter.
- `docs/OPERATIONS.md`: commands, config knobs, troubleshooting.
