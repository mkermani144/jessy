# Jessy → Claude Code Plugin: v1 Plan

Replace Rust jessy with Claude Code plugin. Claude drives Chrome via `claude --chrome`. Skills do crawl + extract + filter + report + learn. LinkedIn only v1.

Rust tree stays for now (nuke postponed). Plugin lives in `plugin/` subdir.

## Goals

- Scan LinkedIn job tabs, score against user prefs, render ranked report as box cards.
- User multi-selects jobs to open for manual apply.
- Learn from dismissals/opens in pattern-triggered batches → update prefs via multiselect prompts.
- Small, LLM-friendly config + prefs files.

## Non-goals v1

- Auto-apply (tab open only).
- Wellfound / fallback platform skills (future).
- OpenAI API (Claude handles extraction natively).
- Migrating Rust `jessy-core` SQLite rows.

## Architecture

Claude = orchestrator. Browser via `claude --chrome`. Shell scripts do DB + rendering only.

```
plugin/
  .claude-plugin/plugin.json
  skills/
    jessy-scan/SKILL.md
    jessy-report/SKILL.md
    jessy-learn/SKILL.md
    platforms/linkedin/SKILL.md
  commands/
    jessy.md            # pipes scan + report
    jessy-scan.md
    jessy-report.md
    jessy-cleanup.md
    jessy-prefs.md
    jessy-config.md
  scripts/
    db.sh               # sqlite3 wrapper: init, insert_job, insert_company, query, cleanup
    render_cards.sh     # box card formatter, bash
    onboard.sh          # first-run: create ~/.jessy, copy examples, prompt inputs
  config/
    config.example.yaml
    preferences.example.md
  PLAN.md               # this file
```

User data at `~/.jessy/`:

```
~/.jessy/
  jessy.db              # sqlite
  config.yaml
  preferences.md
```

## Data model (SQLite)

```sql
CREATE TABLE companies (
  id INTEGER PRIMARY KEY,
  name TEXT UNIQUE NOT NULL,
  size TEXT,
  summary TEXT
);

CREATE TABLE jobs (
  url TEXT PRIMARY KEY,
  company_id INTEGER REFERENCES companies(id),
  title TEXT NOT NULL,
  desc TEXT,                    -- short summary, caveman-ish
  req_hard TEXT,                -- JSON array
  req_nice TEXT,                -- JSON array
  platform TEXT NOT NULL,       -- 'linkedin'
  score INTEGER,                -- 0..100
  rationale TEXT,               -- 1-line why
  user_action TEXT,             -- opened | dismissed | null
  ts INTEGER NOT NULL           -- unix
);

CREATE INDEX jobs_ts ON jobs(ts);
CREATE INDEX jobs_score ON jobs(score);
```

`status` is derived: `score >= threshold_match` ⇒ match. Not stored.

Learning state table:

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
-- keys: jobs_since_last_learn (int), next_cadence_idx (int)
```

## Config (`~/.jessy/config.yaml`)

```yaml
threshold_match: 70           # score >= → match (full card)
threshold_low_show: 30        # score >= → compact list entry; below → "+N ignored"
cleanup:
  max_age_days: 30
  max_rows: 5000
  prompt_when_over: 4000      # suggest /jessy-cleanup when db hits this
learning:
  cadence: [20, 30, 40, 50]   # jobs since last learn → next trigger; last value plateaus
linkedin:
  startup_urls: []            # filled during onboarding
  max_pages: 5
  skip_title_keywords: []     # optional title prefilter (cheap skip before open)
```

Config is both user-editable and LLM-writable. Learning skill updates thresholds only on explicit user ask.

## Preferences (`~/.jessy/preferences.md`)

Sectioned caveman markdown. Jessy reads every scan. Learning skill rewrites with user consent.

```markdown
# Jessy Preferences

## Dealbreakers
- Java primary stack
- on-site NL only
- visa sponsor required

## Dislikes
- 10+ yrs experience demand
- big banks
- finance domain

## Likes
- Rust, TS
- remote EU
- small/mid startups
- applied ML, infra

## Notes
- free text, any extra nuance
```

LLM parses sections for scoring. Prose inside sections OK but bullets preferred.

## Scoring

Per job, Claude compares `req_hard` + `req_nice` + `desc` + company against prefs:

| signal | hard req | nice req |
|---|---|---|
| dealbreaker match | score = 0 (auto not_match) | score = 0 |
| dislike match | -25 | -8 |
| like match | +20 | +8 |
| unmentioned | 0 | 0 |

Base = 50. Clamp [0, 100]. Rationale = 1 short line citing top reasons.

Dealbreaker override: any dealbreaker hit → force score 0 regardless.

## Flows

### Onboarding (first run of any command if `~/.jessy/` missing)

1. `onboard.sh` creates `~/.jessy/`.
2. Describe plugin briefly (caveman).
3. Prompt for LinkedIn search URLs (paste list).
4. Prompt for dealbreakers (quick list).
5. Prompt for likes (quick list).
6. Write `config.yaml`, `preferences.md`, init `jessy.db`.

### `/jessy-scan`

1. Ensure `~/.jessy/` exists (else onboard).
2. Read prefs + config.
3. Use `claude --chrome` to list open LI job tabs.
4. For each LI search tab:
   - Parse list of job cards (selectors from `platforms/linkedin` skill).
   - For each card:
     - Apply title prefilter (`skip_title_keywords`) → skip outright.
     - Check `jobs.url` in DB → skip if seen.
     - Open detail tab via chrome.
     - Extract: title, company (+ size, summary), desc, req_hard, req_nice.
     - Upsert company, insert job with score + rationale.
     - Close detail tab.
5. Walk up to `linkedin.max_pages` per search tab (repeat-page stop if same list).
6. Bump `meta.jobs_since_last_learn`.
7. Print: `scanned N new; M match; K low; L ignored`.

### `/jessy-report`

1. Query last scan's jobs (or all with `user_action IS NULL`).
2. Sort desc by score.
3. Render:
   - `score >= threshold_match` → full box card (render_cards.sh).
   - `threshold_low_show <= score < threshold_match` → compact line: `• {title} — {rationale}  {url}`.
   - `score < threshold_low_show` → tail: `+N more ignored`.
4. AskUserQuestion multiselect: "Open in Chrome:" options = match cards + low list entries (label = `{title} — {short summary}`).
5. For picks: mark `user_action='opened'`, use `claude --chrome` to open tabs.
6. Unpicked match/low entries → `user_action='dismissed'`.
7. If `jobs_since_last_learn >= cadence[next_cadence_idx]` → invoke `jessy-learn`.

### `/jessy` (alias)

Runs `/jessy-scan` then `/jessy-report`.

### `/jessy-learn`

1. Mine last ~50 dismissed + opened jobs.
2. Cluster by signal (stack, seniority, domain, location, company size).
3. For each cluster where N≥3 and lopsided ratio → candidate pattern.
4. AskUserQuestion multiselect: "Which are true?":
   - "dislike Java roles (4 skips, 0 opens)"
   - "10+ yrs experience turns me off (3 skips)"
   - "strong like for remote EU (5 opens)"
   - "ignore all"
5. For each picked → append/update `preferences.md` under correct section.
6. Reset `jobs_since_last_learn = 0`, bump `next_cadence_idx` (clamp to last).

### `/jessy-cleanup`

1. Read config cleanup section.
2. Delete jobs older than `max_age_days` AND `user_action` set.
3. If row count still > `max_rows`, delete oldest `user_action IS NOT NULL` rows until under limit.
4. Never auto-delete rows with `user_action IS NULL` (unseen by user).
5. Print: "pruned X; now Y rows".

Auto-suggest cleanup when `SELECT COUNT(*) FROM jobs > prompt_when_over` during scan.

### `/jessy-prefs` / `/jessy-config`

Open file in `$EDITOR` via shell. Tiny command bodies.

## Platform skill: LinkedIn

`skills/platforms/linkedin/SKILL.md` documents for Claude:

- URL patterns for search vs detail.
- Selectors/landmarks for: job card list, title, company link, location, description section, requirements heading variants ("Qualifications", "Requirements", "What you'll do"), nice-to-have variants ("Nice to have", "Bonus", "Plus").
- Pagination hint (next page button / scroll).
- Same-list detection for repeat-page stop (compare first N urls).
- Company page: size, about-section for summary.

Activation: when Claude sees `linkedin.com/jobs/` URL.

## Report card format

Box card, reuse overall style from current Rust jessy. Fields differ — use:

```
╭─ [MATCH 87] Senior Applied Researcher ──────────────────────────────────────╮
│ Summary: ai leader; recsys; agentic workflows; ml/nlp/llm; prod eng         │
│ Must: Python, PyTorch, big data, cloud, NLP, LLM                            │
│ Nice: Scala, A/B testing, MLOps                                             │
│ Company: eBay — 10k+ — global ecommerce marketplace                         │
│ Why: strong Rust-adjacent ML infra; matches "applied ML" like               │
│ Link: https://www.linkedin.com/jobs/view/4402476780                         │
╰─────────────────────────────────────────────────────────────────────────────╯
```

Header tag `[MATCH S]` where S = score. Low-score compact line:

```
• Backend Eng @ FooBank — score 42 — Java primary + finance dislike  https://...
```

Tail:

```
+23 more non-match jobs ignored
```

## Scripts

Shell (bash + sqlite3 + jq). No Python. No deps beyond macOS defaults + `sqlite3` + `jq`.

- `db.sh` commands: `init`, `upsert_company`, `insert_job`, `mark_action`, `query_report`, `cleanup`, `count`, `meta_get`, `meta_set`.
- `render_cards.sh` stdin = JSON lines of jobs, stdout = cards + compact list + tail.
- `onboard.sh` interactive, writes initial files.

## Commands (`commands/*.md`)

Each is a short prompt telling Claude to invoke the matching skill. Example `commands/jessy.md`:

```markdown
Run jessy: scan new LinkedIn jobs, then render the ranked report.

1. Invoke jessy-scan skill.
2. Invoke jessy-report skill.
```

## Plugin manifest

`.claude-plugin/plugin.json`:

```json
{
  "name": "jessy",
  "version": "0.1.0",
  "description": "Local read-only LinkedIn job scanner with learning prefs",
  "skills": ["skills/jessy-scan", "skills/jessy-report", "skills/jessy-learn", "skills/platforms/linkedin"],
  "commands": ["commands/jessy.md", "commands/jessy-scan.md", "commands/jessy-report.md", "commands/jessy-cleanup.md", "commands/jessy-prefs.md", "commands/jessy-config.md"]
}
```

(Exact schema TBD at impl time — check Claude Code plugin docs.)

## Install

Dev: symlink `plugin/` → `~/.claude/plugins/jessy/`. Later: marketplace ref.

## Open / deferred

- Wellfound + fallback platform skills → v2.
- Apply automation → v3 (gated, explicit opt-in).
- Migrating any Rust SQLite rows → skip unless user asks.
- Nuke Rust tree → after plugin proven.

## Implementation order

1. Plugin manifest + minimal `/jessy-config` + `/jessy-prefs` + onboarding.
2. `db.sh` + schema init.
3. LinkedIn platform skill (selectors only).
4. `jessy-scan` skill with title prefilter + seen-skip.
5. Scoring + `rationale`.
6. `render_cards.sh` + `jessy-report` skill.
7. AskUserQuestion multiselect + open tabs.
8. `jessy-learn` + pattern mining.
9. `jessy-cleanup` + auto-suggest.
10. Package + install docs.

Ship 1–7 as MVP; 8–10 polish.
