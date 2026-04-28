# PLAN-new.md

# Jessy CDP Scan Rewrite

## Problem

Current scan lets Claude drive Chrome card-by-card. One LinkedIn tab can burn
~50% of a 5h usage window and still not finish. This architecture is dead.

## Goal

- Replace browser-driving LLM scan with deterministic CDP/Playwright scan.
- Preserve semantic LLM scoring, but batch it over compact extracted JSON.
- Guarantee no silent card loss.
- Keep normal user browser untouched.

## Hard Decisions

- Do not use Claude-in-Chrome for scan.
- Do not use LLM subagents to control browser.
- Use Chrome Beta dedicated to Jessy.
- Auto-start Chrome Beta for scans.
- Use configured LinkedIn URLs only.
- Ignore existing browser tabs.
- Close scan tabs after scan.
- Preserve `linkedin.max_pages`; default remains `2`.
- Retry broken rows automatically at next scan start.
- Failed/review rows still count as attempted boundary.
- Store failure artifacts only, not every-card artifacts.
- LLM scoring stays batch-based.

## Browser

Use Chrome Beta with Jessy-owned profile:

- app: Google Chrome Beta
- profile dir: `~/.jessy/chrome-beta-profile`
- CDP port: `9223`
- CDP URL: `http://127.0.0.1:9223`

Scan auto-start behavior:

1. Check CDP endpoint.
2. If not live, launch Chrome Beta with remote debugging and Jessy profile.
3. Wait up to ~10s for CDP.
4. If LinkedIn is not logged in, stop with login instruction.
5. Never kill/restart Chrome Beta automatically.
6. If port conflict/profile mismatch, print diagnostic. No auto-kill.

## Scan Flow

1. Retry queue first:
   - statuses: `needs_review`, `extract_failed`/`failed` retryable rows.
   - cap: add config `retry.max_per_scan`, default `10`.
   - Retry uses same CDP extractor.
   - Success moves row to extracted/scored flow.
   - Failure increments retry count and keeps review/failure state.

2. For each configured `linkedin.startup_urls` URL:
   - open fresh controlled page in Chrome Beta.
   - scan only this page/tab.
   - close controlled tab after URL scan.
   - if URL does not expose a detectable job-card list, persist/print
     `unsupported_url`; no silent skip.

3. For each page up to `linkedin.max_pages`:
   - materialize visible job cards.
   - derive canonical URL: `https://www.linkedin.com/jobs/view/<id>`.
   - read card fields: title, company, location, badges/tags, snippet.
   - walk top-down.
   - first attempted URL stops this configured URL/feed.
   - LinkedIn `viewed` is ignored.
   - every discovered unattempted card gets an attempt row before detail work.
   - extract detail via CDP.
   - if CDP extraction is incomplete/vague, run cheap LLM fallback on captured
     page text only. Fallback must not control browser.
   - if still broken, persist `needs_review` with artifact path.

4. Emit extracted jobs as JSONL using current extractor schema.

5. Claude batch-scores extracted JSONL against `preferences.md`.

6. Persist scored jobs in bulk and update attempt status.

7. Print summary:
   - `scanned N new; S scored; R needs_review; F failed; M match; K low; L ignored`

## No Silent Loss Contract

Every discovered card must end as one of:

- `scored`
- `needs_review`
- `blocked_auth`
- `removed`
- `unsupported_url`
- `failed`

Do not drop malformed/incomplete cards into logs only.

`needs_review` cards:

- are stored in DB.
- are retried automatically on next scan.
- appear in report/manual-review flow until opened/dismissed or scored.

Failure artifacts:

- only for failed/review cases.
- path: `~/.jessy/artifacts/<job-id>-<ts>/`
- include:
  - captured visible text
  - screenshot
  - URL
  - reason/error

## Output Schema

`linkedin_cdp.js scan` emits existing extractor-schema JSONL only.

Success:

```json
{
  "status": "ok",
  "url": "https://www.linkedin.com/jobs/view/123",
  "lang": "en",
  "title": "Staff Backend Engineer",
  "company": "Acme",
  "company_size": "unknown",
  "location": "remote US",
  "seniority": "staff",
  "employment": "full_time",
  "salary": "unknown",
  "visa": "unknown",
  "req": ["8 years backend", "rust", "distributed systems"],
  "nice": ["kubernetes"],
  "summary": ["Build backend services", "Own production systems"],
  "evidence": ["Remote - United States", "8+ years backend engineering"]
}
```

Failure/review:

```json
{
  "status": "failed",
  "url": "https://www.linkedin.com/jobs/view/123",
  "reason": "detail_not_loaded",
  "artifact_dir": "/Users/.../.jessy/artifacts/123-1710000000"
}
```

Allowed statuses:

- `ok`
- `partial`
- `failed`

Allowed failure reasons:

- `timeout`
- `load_failed`
- `detail_not_loaded`
- `auth_wall`
- `removed`
- `invalid_url`
- `not_job`
- `unsupported_url`

Field caps:

- `req`: max 10
- `nice`: max 5
- `summary`: max 4
- `evidence`: max 4
- each string: max 120 chars
- no full job description
- no repeated boilerplate

Enums:

- `seniority`: `intern|junior|mid|senior|staff|principal|exec|unknown`
- `employment`: `full_time|contract|part_time|internship|unknown`

## Scoring

Claude keeps scoring because prefs are semantic.

Scoring must be batch-based:

- read `preferences.md` once.
- score extracted JSONL in one call per batch.
- chunk large batches, e.g. 25 jobs.
- no per-job scoring prompts.

Scored output should map to existing report schema:

- `url`
- `title`
- `desc`
- `req_hard`
- `req_nice`
- `company_name`
- `company_size`
- `company_summary`
- `score`
- `rationale`

Persist scored rows in one transaction/bulk helper.

## CDP Extractor Strategy

Use Node + Playwright.

Implement one script with subcommands:

- `plugin/scripts/linkedin_cdp.js scan`
- `plugin/scripts/linkedin_cdp.js retry`
- `plugin/scripts/linkedin_cdp.js open`
- `plugin/scripts/linkedin_cdp.js status`

Selector strategy:

- Hybrid, biased to text-region heuristics.
- Use selectors only for:
  - card anchors / job IDs
  - obvious title/company/location fields
  - `See more`
  - pagination/scroll controls
- Extract broad visible text from detail pages.
- Parse requirements from text headings.
- Avoid brittle deep LinkedIn class chains.

Cheap LLM fallback:

- input: canonical URL, card fields, captured detail text, extractor schema.
- output: extractor JSON only.
- no prefs.
- no scoring.
- no browser/tool access.

## DB Changes

Extend `job_attempts` as needed:

- `status`
- `retry_count`
- `last_error`
- `artifact_dir`
- `extraction_json`
- `score`
- `rationale`
- timestamps

Statuses should support:

- `started`
- `ok`
- `partial`
- `scored`
- `needs_review`
- `blocked_auth`
- `removed`
- `unsupported_url`
- `failed`

Keep existing `jobs` table/report format unless truly necessary.

## Config Changes

Add:

```yaml
browser:
  channel: chrome-beta
  cdp_url: http://127.0.0.1:9223
  profile_dir: ~/.jessy/chrome-beta-profile

retry:
  max_per_scan: 10
```

Keep:

```yaml
linkedin:
  startup_urls: []
  max_pages: 2
  skip_title_keywords: []
```

## Permissions

Avoid approval spam.

Plugin settings must allow literal script calls:

- `Bash(*/scripts/linkedin_cdp.js*)`
- existing DB/render/onboard helpers

Skill instructions must forbid:

- `$DB ...`
- shell functions like `skip_one`
- shell loops around DB/script calls
- command substitution wrappers for core scan persistence

Use one literal script invocation per Bash tool call where possible.

## Report Changes

Report must not hide review/failure rows.

Add a top section for `needs_review` rows:

- title/company/url if known
- reason
- artifact path if present
- open action in Chrome Beta

Manual action can dismiss/open review rows.

## Acceptance Checks

- One configured URL with 20 cards does not invoke Claude-in-Chrome.
- No extractor subagents control browser.
- Chrome Beta auto-starts and normal Chrome is untouched.
- Configured URLs are opened fresh; existing tabs ignored.
- Scan tabs close after scan.
- `max_pages=2` honored.
- First attempted card stops that configured URL/feed.
- Failed/review cards are persisted, not silently skipped.
- Retry queue runs automatically at next scan start.
- Failure artifacts created only for failed/review rows.
- Extractor emits current schema JSONL.
- Scoring is batch-based, not per job.
- Report surfaces `needs_review` rows.
- Permission prompts do not appear for DB/script loops/functions.
