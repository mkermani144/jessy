---
name: jessy-scan
description: Scan open LinkedIn job tabs in Chrome, score each against the user's preferences, and persist new jobs to ~/.jessy/jessy.db. Use when the user runs /jessy:scan or asks jessy to look for new jobs.
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh*)
  - Bash(test *)
  - Bash(cat *)
  - Read
---

# jessy-scan

Orchestrates a scan pass: enumerate LinkedIn tabs → for each search,
walk pages → for each job card, prefilter + seen-skip → open, extract,
score → upsert company, insert job → close. Bumps learning counter.
Prints summary.

Runs against a live Chrome session via `claude --chrome`. Page semantics
in `skills/platforms/linkedin/SKILL.md` (auto-loads on linkedin URLs).

## Preconditions

1. `~/.jessy/config.yaml` exists. If not, run
   `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh` first, then continue.
2. Chrome session is attached (`claude --chrome`).
3. User is signed into LinkedIn in that Chrome profile.

## Inputs

Read once at start:

- `~/.jessy/config.yaml` → `linkedin.max_pages`, `linkedin.skip_title_keywords`,
  `linkedin.startup_urls`, `cleanup.prompt_when_over`.
- `~/.jessy/preferences.md` → Dealbreakers / Dislikes / Likes / Notes sections,
  for scoring.

## Procedure

### 1. Discover scan tabs

List the open tabs in Chrome. Keep only tabs whose URL matches the
LinkedIn search/collection patterns (see linkedin SKILL.md).

If no LinkedIn tabs are open AND `linkedin.startup_urls` is non-empty,
open each `startup_url` in a new tab and treat those as the scan tabs.

If still none, print "no LinkedIn job tabs to scan" and stop.

### 2. For each scan tab

Track: `page_index = 0`, `prev_first_urls = []`, `pages_walked = 0`.

While `pages_walked < linkedin.max_pages`:

a. Scroll the job-card list to the bottom to materialize all cards.
b. Read the visible job cards: title, canonical URL (`/jobs/view/<id>`).
c. **Same-list stop** — first 3 URLs equal `prev_first_urls` ⇒ stop walking
   this tab (pager did not advance).
d. Update `prev_first_urls` = first 3 URLs.
e. **For each card** (in order):
   - **Title prefilter**: drop if title (case-insensitive, substring)
     matches any `linkedin.skip_title_keywords` value.
   - **Seen-skip**: `db.sh seen <canonical_url>` prints `yes` or `no`
     (always exit 0). `yes` → already stored; skip the card.
   - Click the card (loads detail in right rail).
   - Extract `title`, `company_name`, `desc`, `req_hard`, `req_nice`
     using the linkedin skill's heading rules. Expand any "See more"
     before reading.
   - If a company link is visible, fetch the company page in a new tab
     to get `size` + a one-sentence `summary`. Close that tab.
   - **Score** (rubric below). Build `rationale` (one short line).
   - `cid=$(db.sh upsert_company "<name>" "<size>" "<summary>")`
   - `db.sh insert_job <canonical_url> "$cid" "<title>" "<desc>" \
        '<req_hard_json>' '<req_nice_json>' linkedin <score> "<rationale>"`
   - Track running counts: `new`, `match` (score ≥ `threshold_match`),
     `low` (score in `[threshold_low_show, threshold_match)`),
     `ignored` (score < `threshold_low_show`).
f. Click next-page (or scroll for infinite scroll). Increment
   `pages_walked`. Loop.

### 3. After all tabs

- `db.sh meta_get jobs_since_last_learn` → add `new`, write back via
  `db.sh meta_set jobs_since_last_learn <total>`.
- `db.sh count` → if total > `cleanup.prompt_when_over`, print a hint:
  "DB has X rows; consider /jessy:cleanup".
- Print one-line summary: `scanned N new; M match; K low; L ignored`.

## Scoring rubric

For each job, compare `req_hard` + `req_nice` + `desc` + company summary
against the prefs sections:

| signal              | hard req           | nice req |
|---------------------|--------------------|----------|
| dealbreaker match   | force score = 0    | force score = 0 |
| dislike match       | -25                | -8       |
| like match          | +20                | +8       |
| unmentioned         |  0                 |  0       |

Algorithm:

1. Start `score = 50`.
2. If any dealbreaker matches anywhere (req_hard, req_nice, desc, or
   company), set `score = 0` and stop (rationale must cite the dealbreaker).
3. Otherwise sum deltas across all matched dislikes/likes (each pref bullet
   counts at most once — across hard+nice, take the larger penalty/bonus).
4. Clamp to `[0, 100]`.
5. `rationale` = one short line (≤ ~100 chars) citing the top 1-2 reasons,
   e.g. `Rust + remote EU like; small startup match`. For score-0 cases,
   cite the dealbreaker, e.g. `dealbreaker: Java primary stack`.

Matching is semantic, not literal — "Postgres" matches a "PostgreSQL" like;
"on-site NL only" matches a job that says "must be in Amsterdam office".

## Field formats for db.sh insert_job

- `<url>`: canonical `https://www.linkedin.com/jobs/view/<id>` (strip
  query params; keep only the id).
- `<company_id>`: integer printed by `db.sh upsert_company`.
- `<title>`, `<desc>`, `<rationale>`: plain text. Quote bash-safely.
- `<req_hard>`, `<req_nice>`: JSON arrays of strings, e.g. `["Rust","5+ yrs"]`.
  Use `[]` for empty.
- `<platform>`: literal `linkedin`.
- `<score>`: integer 0-100.

## Error handling

- Login wall on a card → stop scanning that tab, surface to user, continue
  other tabs.
- A card's detail fails to load after a couple retries → skip the card,
  do NOT mark it seen (no DB row), continue.
- `db.sh insert_job` fails → log and continue; partial scans are OK.

## What this skill does NOT do

- Render the report (that's `/jessy:report`, later round).
- Mark `user_action` (that's the report flow).
- Trigger learning (report flow checks the cadence).
- Auto-apply / fill forms.
