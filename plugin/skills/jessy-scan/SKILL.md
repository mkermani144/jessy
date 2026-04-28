---
name: jessy-scan
description: Scan open LinkedIn job tabs in Chrome, extract unseen jobs, score against preferences in the main thread, and persist results to ~/.jessy/jessy.db. Use when the user runs /jessy:scan or asks jessy to look for new jobs.
model: sonnet
effort: low
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh*)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh*)
  - Bash(test *)
  - Read
  - Agent
  - mcp__claude-in-chrome
  - mcp__claude-in-chrome__*
---

# jessy-scan

Normal scan is bounded by Jessy history, not by LinkedIn viewed labels.
Walk each LinkedIn tab/feed top-down. At the first Jessy-attempted card in
that tab/feed, stop lower/older cards and move to the next tab/feed.

Use the custom `jessy-linkedin-extractor` Agent for extraction. It runs on
Haiku with a narrow extraction-only prompt. Extractors receive one URL/card,
no preferences, no rubric, no fit judgment. Run them serialized, one at a
time. Main thread owns matching, scoring, DB writes, learning counters, and
summary output.

Runs against a live Chrome session via `claude --chrome`. Page semantics
live in `skills/platforms/linkedin/SKILL.md`. Per-card Agent input prompt:
`card-task.md`.

## Preconditions

1. `~/.jessy/config.yaml` exists. If missing, run
   `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh`, then continue.
2. Chrome session is attached (`claude --chrome`).
3. User is signed into LinkedIn in that Chrome profile.
4. On first Chrome-extension prompt, tell the user to allow the upcoming
   LinkedIn tab reads. Do not ask again unless Chrome prompts again.

## Inputs

Read once at start:

- Run `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh init` once to migrate old DBs.
- `~/.jessy/config.yaml`:
  - `threshold_match`
  - `threshold_low_show`
  - `linkedin.max_pages`
  - `linkedin.max_new_per_run` (default 20 if missing)
  - `linkedin.skip_title_keywords`
  - `linkedin.startup_urls`
  - `cleanup.prompt_when_over`
- `~/.jessy/preferences.md`: full main-thread preference context.
  Extract bullets under `## Dealbreakers`, `## Dislikes`, `## Likes`.
- `${CLAUDE_PLUGIN_ROOT}/skills/jessy-scan/card-task.md`: per-card input
  contract to send to the `jessy-linkedin-extractor` Agent.

Maintain timers: `discover_ms`, `card_read_ms`, `db_ms`, `extract_ms`,
`score_ms`, `total_ms`.

Maintain `attempted_cache[canonical_url] -> yes|no`.
Maintain `cap_hit = false` and `stop_scan = false`. `new` counts every newly
attempted unattempted card, including skipped, failed, partial, and scored
cards.

## Permission Discipline

Claude Code permission matching is command-shaped. During scan, every Bash
call must start with one literal script path:

- `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh ...`
- `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh ...`
- `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh ...`

Do not use shell variables (`DB=...`, `$DB ...`), shell functions
(`skip_one`), command substitution (`cid=$(...)`), pipes, or shell
`for`/`while` loops around DB work. Use one script invocation per Bash call.
For repeated attempt checks, call `db_scan.sh attempted_many <url...>` once.
For direct skips and scored rows, use `db_scan.sh` compound commands below.

## Procedure

### 1. Discover scan tabs

List Chrome tabs. Keep LinkedIn jobs search / collection tabs per the
linkedin platform skill.

If no LinkedIn scan tabs are open and `linkedin.startup_urls` is non-empty,
open each startup URL in a new tab and treat those as scan tabs.

If still none, print `no LinkedIn job tabs to scan` and stop.

### 2. Walk each scan tab

For each tab while `stop_scan=false`:

- `prev_first_urls = []`
- `pages_walked = 0`
- `stop_this_tab = false`

While `pages_walked < linkedin.max_pages`, `stop_this_tab=false`, and
`stop_scan=false`:

1. Scroll the job-card list to the bottom to materialize cards.
2. Read visible cards in list order:
   - title
   - canonical URL
   - company
   - location
   - badges/tags
   - short visible snippet if present
3. Same-list stop: if first 3 canonical URLs equal `prev_first_urls`, stop
   this tab. Otherwise set `prev_first_urls` to the current first 3.
4. For each visible card, in order:
   - Canonicalize to `https://www.linkedin.com/jobs/view/<id>`; strip query
     params and keep only the ID.
   - Attempt boundary: call
     `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh attempted <canonical_url>` on
     cache miss. If `yes`, set `stop_this_tab=true` and stop lower/older
     cards in this tab/feed. Continue the next tab/feed.
     If checking several visible card URLs at once, use:
     `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh attempted_many <url...>`
   - Ignore LinkedIn `viewed`; it is not a boundary.
   - Scan cap: after confirming the card is unattempted, if
     `new >= linkedin.max_new_per_run`, set `cap_hit=true`,
     `stop_scan=true`, and stop all remaining cards/tabs without writing an
     attempt for this card.
   - Title skip keywords: if title matches any
     `linkedin.skip_title_keywords`, persist with:
     `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh skip_job <url> <company> <title> <snippet> 0 "skip title: <keyword>"`
     Count new and ignored, cache attempted `yes`, continue.
   - Title-only dealbreaker: if a dealbreaker bullet matches the title,
     persist with `db_scan.sh skip_job` and rationale
     `dealbreaker (title): <bullet>`. Count new and ignored, cache attempted
     `yes`, continue.
   - Otherwise persist attempt start:
     `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh attempt_start <url> linkedin`
     Count new and cache attempted `yes`.
   - Use the Agent tool with subagent type `jessy-linkedin-extractor`. Do not
     dispatch another extractor until this one returns.
   - Extractor input:
     - canonical URL
     - card title
     - card company
     - card location
     - card badges/tags
     - card snippet
   - Extractor must return strict JSON only. No markdown. No fit judgment.
   - If extractor reports a mechanical load issue (`timeout`,
     `load_failed`, `detail_not_loaded`), retry once immediately for that
     same card. Do not retry auth/removed/invalid/not_job failures.
   - Persist extraction outcome:
     - `status=ok|partial`: finish attempt with extractor JSON, then score.
     - `status=failed`: finish attempt as `failed`; no normal-scan retry.
       Failed rows count as attempted and future scan boundary.
   - Score in the main thread from extractor JSON + preferences only.
   - Insert the scored row with one call:
     `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh score_job <url> <company> <company_size> <title> <desc> <req_json> <nice_json> <score> <rationale> <extract_json>`
   - Tally:
     - `new`: every newly attempted unattempted card, including skipped,
       failed, partial, and scored cards
     - `match`: score >= `threshold_match`
     - `low`: score in `[threshold_low_show, threshold_match)`
     - `ignored`: score < `threshold_low_show` or failed/skip attempt
5. If not stopped, click next-page or continue infinite scroll. Increment
   `pages_walked`.

### 3. Main Scoring

Extractor JSON is the only job evidence. Main thread may infer domain, role
shape, fit, misfit, and uncertainty from these fields:

- `title`
- `company`
- `company_size`
- `location`
- `seniority`
- `employment`
- `salary`
- `visa`
- `req`
- `nice`
- `summary`
- `evidence`

Scoring algorithm:

1. Start `score = 50`.
2. If any dealbreaker matches `req`, `nice`, `summary`, `location`,
   `employment`, `visa`, title, or evidence: force `score = 0`.
3. Otherwise apply each preference bullet once:
   - dislike in `req` / `summary`: `-25`
   - dislike in `nice` / weak evidence: `-8`
   - like in `req` / `summary`: `+20`
   - like in `nice` / weak evidence: `+8`
4. Clamp score to `[0, 100]`.
5. Choose decision:
   - `accept`: score >= `threshold_match`
   - `maybe`: score >= `threshold_low_show`
   - `reject`: score < `threshold_low_show`
   - `defer`: useful data missing but not failed
6. Rationale: one line, <= 100 chars, citing top 1-2 reasons.

## Extractor Output

Strict JSON object:

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

Status values:

- `ok`: useful detail loaded.
- `partial`: some useful detail loaded, key fields missing.
- `failed`: no useful detail.

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

## DB Writes

Use:

- `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh attempted <url>` for boundary checks.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh attempted_many <url...>` for
  batch boundary checks without shell loops.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh attempt_start <url> linkedin` before
  extraction.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh skip_job ...` for title skips.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh score_job ...` for scored rows.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh fail_attempt ...` for failed
  extraction.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh bump_learn <new>` after
  all tabs.

For failed extraction, do not insert a reportable job unless there is enough
card data to create a useful ignored row. Always finish the attempt as
`failed`.

## After All Tabs

- If `new > 0`: call
  `${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh bump_learn <new>`.
- Check row count. If over `cleanup.prompt_when_over`, print:
  `DB has X rows; consider /jessy:cleanup`.
- Print: `scanned N new; M match; K low; L ignored`.
  If `cap_hit=true`, print:
  `scanned N new; M match; K low; L ignored; cap hit`.
- Print timing:
  `timing discover=Xms card_read=Yms extract=Zms score=Ams db=Bms total=Cms`.

## Forbidden In Normal Scan

- No per-card judge subagents.
- No parallel extractor batches.
- No lean/full/deepen flow.
- No preferences or scoring rubric in extractor prompts.
- No company-page browsing.
- No extra tabs during extraction.
- No LinkedIn `viewed` boundary logic.
