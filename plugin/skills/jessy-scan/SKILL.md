---
name: jessy-scan
description: Scan open LinkedIn job tabs in Chrome, score each against the user's preferences, and persist new jobs to ~/.jessy/jessy.db. Use when the user runs /jessy:scan or asks jessy to look for new jobs.
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh*)
  - Bash(test *)
  - Read
  - Agent
  - mcp__claude-in-chrome
  - mcp__claude-in-chrome__*
---

# jessy-scan

Orchestrates a scan pass: enumerate LinkedIn tabs → for each search,
walk pages → for each visible card, run cheap prefilters → **stage 1
card triage** (`skip` / `maybe` / `likely`) → **stage 2 detail
extraction** (`lean` for `maybe`, `full` for `likely`) → optional
`deepen` retry for lean ambiguous cards → main thread writes DB rows.
Bumps learning counter. Prints count + phase timing summary.

Why two-stage: per-card DOM and company-page fetch are the bulk of token
cost. Spend almost nothing on obvious losers, spend a little on maybes,
spend full tokens only on promising cards.

Runs against a live Chrome session via `claude --chrome`. Page semantics
in `skills/platforms/linkedin/SKILL.md` (auto-loads on linkedin URLs).
Per-card subagent prompt template: `card-task.md` in this skill dir.

## Preconditions

1. `~/.jessy/config.yaml` exists. If not, run
   `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh` first, then continue.
2. Chrome session is attached (`claude --chrome`).
3. User is signed into LinkedIn in that Chrome profile.
4. On first Chrome-extension prompt, tell the user to allow access for the
   upcoming LinkedIn tab reads. Do not ask again unless Chrome prompts again.

## Inputs

Read once at start:

- `~/.jessy/config.yaml` → `threshold_match`, `threshold_low_show`,
  `linkedin.max_pages`, `linkedin.skip_title_keywords`,
  `linkedin.startup_urls`, `cleanup.prompt_when_over`.
- `~/.jessy/preferences.md` → Dealbreakers / Dislikes / Likes / Notes sections.
  Hold the **full text** in a variable (`prefs_text`) — you will pass it
  to stage-2 subagents. Also extract bullets under:
  - `## Dealbreakers` → `dealbreakers`
  - `## Dislikes` → `dislikes`
  - `## Likes` → `likes`

Also initialize:

- Phase timers: `scan_start`, plus cumulative `discover_ms`,
  `card_read_ms`, `prefilter_ms`, `stage2_ms`, `db_ms`.
- Small in-memory caches for this run:
  - `seen_cache[canonical_url] -> yes|no`
  - `company_known_cache[company_name_lower] -> true|false`

## Procedure

### 1. Discover scan tabs

Time this section as `discover_ms`.

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
   Time card DOM reads as `card_read_ms`.
c. **Same-list stop** — first 3 URLs equal `prev_first_urls` ⇒ stop walking
   this tab (pager did not advance).
d. Update `prev_first_urls` = first 3 URLs.
e. **Cheap prefilter + stage-1 triage** (no detail click, no subagent).
   Time this cheap decision work as `prefilter_ms`.
   For each visible card, in order:
   - Canonicalize the URL to `https://www.linkedin.com/jobs/view/<id>`
     (strip query params; keep only the id).
   - **Seen-skip**: look up `canonical_url` in `seen_cache`; on miss call
     `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh seen <canonical_url>` once and
     cache `yes|no`. `yes` ⇒ skip card.
   - **Skip-title-keywords**: title (case-insensitive substring) matches
     any `linkedin.skip_title_keywords` value ⇒ skip card.
   - **Title-only dealbreaker prefilter**: for each bullet in
     `dealbreakers`, case-insensitive substring match against the
     card title alone. On match, write a score=0 row directly — no
     detail click, no subagent — using the card title and the matched
     bullet as the rationale:
     ```
     cid=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh upsert_company "<card_company_name>" "" "")
     ${CLAUDE_PLUGIN_ROOT}/scripts/db.sh insert_job <canonical_url> "$cid" "<card_title>" "" '[]' '[]' \
       linkedin 0 "dealbreaker (title): <bullet>"
     ```
     On successful insert, set `seen_cache[canonical_url]=yes`.
     Tally as `ignored`. Move on.
   - Capture any cheap card metadata visible without opening detail:
     `card_company_name`, `card_location`, visible badges/tags, and a
     short snippet/preview if LinkedIn shows one on the card.
   - **Stage 1 route** using card-only evidence:
     - `skip`: only for strong negatives visible already on the card.
       Use sparingly. Good examples: explicit on-site location conflict,
       explicit seniority/domain/company-name dislike, or multiple clear
       negatives with no visible like. On `skip`, insert a partial row
       directly:
       ```
       cid=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh upsert_company "<card_company_name>" "" "")
       ${CLAUDE_PLUGIN_ROOT}/scripts/db.sh insert_job <canonical_url> "$cid" "<card_title>" "<card_snippet_or_empty>" \
         '[]' '[]' linkedin <score_below_threshold_low_show> "stage1 skip: <reason>"
       ```
       On successful insert, set `seen_cache[canonical_url]=yes`.
       Choose an integer score strictly below `threshold_low_show`
       (typically `threshold_low_show - 1`, clamped at 0). Tally as
       `ignored`. Move on.
     - `likely`: at least one clear positive card signal and no strong
       negative. Examples: title/stack/location/company clearly align.
     - `maybe`: mixed, weak, or insufficient evidence. On uncertainty,
       prefer `maybe`, not `skip`.
   - Add `likely` cards to `likely_list`. Add `maybe` cards to
     `maybe_list`. Keep `canonical_url`, `card_title`,
     `card_company_name`, cheap card metadata, and a short
     `route_reason`.
f. **Dispatch stage-2 subagents**.
   - `likely_list` → call `card-task.md` with `scan_mode=full`
   - `maybe_list` → call `card-task.md` with `scan_mode=lean`
   To parallelize, issue multiple Task calls in a single message
   (small batches, e.g. 3-5 at a time, to stay polite to LinkedIn).
   For each card pass:
   - `canonical_url`
   - `card_title`
   - `card_company_name`
   - `card_location`
   - `card_badges`
   - `card_snippet`
   - `route_reason`
   - `scan_mode` = `lean` or `full`
   - `prefs_text` (full prefs)
   - `company_already_known` = cached result for normalized
     `card_company_name`; on miss call
     `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh company_exists
     "<card_company_name>"` once and cache it (`yes` → `true`)
   - `scoring_rubric` = the rubric block below
   The subagent returns one JSON line. See `card-task.md` for the exact
   shapes.
   Time all Task dispatch + waits as `stage2_ms`; time only DB calls as
   `db_ms`.
g. **Apply each subagent result** in the main thread:
   - On `error: login_wall` ⇒ stop scanning this tab; surface to user;
     continue other tabs.
   - On `error: detail_load_failed` ⇒ skip (no DB row, do not mark seen).
   - On `decision: deepen` ⇒ immediately re-dispatch that same card once
     with `scan_mode=full`. Do not write a DB row yet. Use the same
     `canonical_url`, card metadata, prefs, and route reason.
   - Otherwise:
     ```
     cid=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh upsert_company "<company_name>" "<company_size>" "<company_summary>")
     ${CLAUDE_PLUGIN_ROOT}/scripts/db.sh insert_job <url> "$cid" "<title>" "<desc>" \
       '<req_hard_json>' '<req_nice_json>' linkedin <score> "<rationale>"
     ```
     If `company_already_known` was true (size/summary may be empty),
     `upsert_company` is still called (it preserves existing values via
     `COALESCE NULLIF`). Lean-mode final rows typically leave
     `company_size` / `company_summary` empty.
     On successful insert, set `seen_cache[url]=yes` and
     `company_known_cache[company_name_lower]=true`.
   - Tally counts: `new` (inserted), `match` (score ≥ `threshold_match`),
     `low` (score in `[threshold_low_show, threshold_match)`),
     `ignored` (score < `threshold_low_show`).
h. Click next-page (or scroll for infinite scroll). Increment
   `pages_walked`. Loop.

### 3. After all tabs

- If `new > 0`: `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh meta_get
  jobs_since_last_learn` → add `new`, write back via
  `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh meta_set jobs_since_last_learn
  <total>`. If `new == 0`, skip both calls.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh count` → if total >
  `cleanup.prompt_when_over`, print a hint:
  "DB has X rows; consider /jessy:cleanup".
- Print one-line summary: `scanned N new; M match; K low; L ignored`.
- Print one-line timing summary:
  `timing discover=Xms card_read=Yms prefilter=Zms stage2=Ams db=Bms total=Cms`.

## Subagent prompt template (per card)

Use the Task tool. Concrete instructions live in
`${CLAUDE_PLUGIN_ROOT}/skills/jessy-scan/card-task.md` — read it once at
scan start and inline it into each Task prompt, with the per-card inputs
substituted.

Inputs the subagent receives:

- `canonical_url` — `https://www.linkedin.com/jobs/view/<id>`
- `card_title` — title from the search list (informational)
- `card_company_name`, `card_location`, `card_badges`, `card_snippet`
  — cheap card metadata already visible on the search list
- `route_reason` — one short line explaining why stage 1 chose this path
- `scan_mode` — `lean` or `full`
- `prefs_text` — full preferences.md text (Dealbreakers / Dislikes /
  Likes / Notes)
- `company_already_known` — `true` or `false` (skip company page fetch
  when `true`)
- `scoring_rubric` — the rubric block below

The subagent must return **exactly one JSON line**, no prose.

Final row shape:

```
{"url":"<canonical_url>","title":"<str>","company_name":"<str>","company_size":"<str>","company_summary":"<str>","desc":"<str>","req_hard":["..."],"req_nice":["..."],"score":<int 0-100>,"rationale":"<str>"}
```

Lean-mode deepen sentinel:

```
{"url":"<canonical_url>","decision":"deepen"}
```

These keys map 1:1 to `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh upsert_company`
(`company_name`, `company_size`, `company_summary`) and
`${CLAUDE_PLUGIN_ROOT}/scripts/db.sh insert_job` (`url`,
`title`, `desc`, `req_hard`, `req_nice`, `score`, `rationale`).
When `company_already_known` is `true`, the subagent returns empty
`company_size` / `company_summary` (the existing row is preserved).

Error sentinels:
- `{"url":"<canonical_url>","error":"detail_load_failed"}` — skip card.
- `{"url":"<canonical_url>","error":"login_wall"}` — stop tab.

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

## Field formats for `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh insert_job`

- `<url>`: canonical `https://www.linkedin.com/jobs/view/<id>` (strip
  query params; keep only the id).
- `<company_id>`: integer printed by
  `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh upsert_company`.
- `<title>`, `<desc>`, `<rationale>`: plain text. Quote bash-safely.
- `<req_hard>`, `<req_nice>`: JSON arrays of strings, e.g. `["Rust","5+ yrs"]`.
  Use `[]` for empty.
- `<platform>`: literal `linkedin`.
- `<score>`: integer 0-100.

## Error handling

- Subagent returns `error: login_wall` → stop scanning that tab, surface
  to user, continue other tabs.
- Subagent returns `error: detail_load_failed` → skip the card, do NOT
  mark it seen (no DB row), continue.
- Subagent returns malformed JSON → skip the card, log briefly, continue.
- `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh insert_job` fails → log and
  continue; partial scans are OK.

## What this skill does NOT do

- Render the report (that's `/jessy:report`, later round).
- Mark `user_action` (that's the report flow).
- Trigger learning (report flow checks the cadence).
- Auto-apply / fill forms.
