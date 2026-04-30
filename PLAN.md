# Jessy Plugin Refactor Plan

## Goal

Refactor Jessy plugin around context isolation.

Primary rule: main agent never sees job payloads, HTML, extracted JSON,
report cards, or script rows. It sees only compact stage receipts.

Speed is secondary. Lower context growth and lower model waste are the goal.

## Core Shape

Use few deep custom agent types, but many short-lived invocations.

Do not create shallow agents per tiny step. Agent startup/prompt context has
cost. Split only when one of these changes:

- tool boundary changes
- model boundary changes
- payload/context boundary changes
- retry/restart boundary changes

Data moves through DB/temp files, not chat.

Main agent is a supervisor only:

```json
{ "run_id": 42, "next": "judge", "status": "ok" }
```

Workers return compact receipts only:

```json
{
  "agent": "judge-worker",
  "status": "ok",
  "run_id": 42,
  "claimed": 5,
  "wrote": 5,
  "failed": 0,
  "done": false
}
```

## Agent Types

### `jessy-browser-worker`

Model: `haiku`.

Tools: Chrome MCP, DB scripts, Read small config files.

Owns browser-facing work:

- open startup URLs if needed
- discover list tabs/cards
- capture compact list snapshots
- title/history prefilter
- fetch detail snapshots for kept jobs

Invocation unit:

- one browser queue item, or one small browser batch
- exits after writing DB/files

Execution:

- serial by default
- Chrome state/tool contention makes parallel browser workers wasteful

Returns only counts and next-state.

### `jessy-judge-worker`

Model: `sonnet`.

Tools: DB scripts, Read snapshot refs only.

Owns semantic judgment:

- read natural-language preferences
- read bounded detail snapshots
- extract structured fields
- score against preferences
- write extraction JSON, decision, score, rationale

Invocation unit:

- claim N detail snapshots, default 5
- reduce to 1 if snapshot text is large
- exits after writing results

Execution:

- serial initially
- parallel later only after atomic claim scripts are reliable

Why Sonnet:

- preferences are natural language
- scoring is semantic, not keyword-only
- Haiku may be acceptable later for extraction-only or fallback tiers, but
  default judge should optimize correctness per token burned

### `jessy-report-worker`

Model: `haiku`.

Tools: report scripts, DB scripts, optional tmux/less helper.

Owns:

- prepare report snapshot outside chat
- show only temp path/prompt
- consume choices
- mark opened/dismissed

Invocation unit:

- `prepare`: one report snapshot and prompt
- `consume`: one user choice string and existing snapshot

Returns compact summary only.

Pause boundary:

- subagent should not hold open waiting for user input
- `prepare` returns `{status:"paused", pause_token:"...", prompt:"..."}`
- main asks the user for choices
- main passes only the raw choice text and pause token to `consume`
- main still never sees report rows/cards/index maps

### `jessy-ops-worker`

Model: `haiku`.

Tools: DB/config/onboard/cleanup scripts.

Owns:

- onboarding checks
- run creation
- queue creation
- cleanup
- learn cadence checks

Invocation unit:

- one operational task

No bulk stdout.

### `jessy-learn-worker`

Model: `sonnet`.

Tools: DB scripts, Read/Edit preferences only.

Owns:

- read recent acted rows
- infer preference candidates
- propose or apply accepted preference edits
- update learning cadence meta

Invocation unit:

- one cadence-triggered learning pass

Reason for separate worker:

- keeps normal ops prompt/model cheap
- avoids loading learning instructions into every operational task
- natural-language preference induction needs stronger judgment than cleanup

## Pipeline

### 1. Prepare

Worker: `jessy-ops-worker`.

Reads:

- `~/.jessy/config.yaml`
- `~/.jessy/preferences.md` existence
- DB schema/meta
- platform enabled flags

Writes:

- `runs`
- initial `stage_items`
- `stage_events`

Does not read job data.

### 2. Browser Scan

Worker: repeated `jessy-browser-worker` invocations.

Reads:

- Chrome tabs
- config startup URLs
- DB history/attempts
- pending browser queue items

Writes:

- `page_snapshots`
- `job_seeds`
- skip attempts for title/history skips
- `detail_snapshots`
- `stage_items` status updates
- compact errors

Notes:

- list snapshots should store normalized visible text and selected links, not
  full page HTML
- raw HTML may be stored only as capped/compressed debug payload
- detail snapshots should be bounded text/DOM subset

### 3. Judge

Worker: repeated `jessy-judge-worker` invocations.

Reads:

- preferences
- claimed `detail_snapshots`
- card metadata from `job_seeds`

Writes:

- `job_attempts.extraction_json`
- `job_attempts.score`
- `job_attempts.rationale`
- `jobs`
- `stage_items` status updates

Notes:

- no Chrome tools
- no report rendering
- no full descriptions in stdout

### 4. Report

Worker: `jessy-report-worker`.

Reads:

- unseen scored `jobs`

Writes:

- temp report snapshot
- temp index map
- `stage_events`

Chat output:

- temp path/prompt only

### 5. Consume

Worker: `jessy-report-worker`.

Reads:

- report snapshot
- user pick text
- pause token

Writes:

- `jobs.user_action = opened|dismissed`
- cadence meta

Chat output:

```text
opened N; dismissed M; unseen 0.
```

### 6. Learn / Cleanup

Workers: `jessy-ops-worker`, `jessy-learn-worker`.

Reads:

- recent acted rows
- preferences
- cleanup policy

Writes:

- preference proposal or accepted prefs patch
- cleanup deletions
- meta updates

Learn uses `jessy-learn-worker` on Sonnet. Cleanup uses
`jessy-ops-worker` and should be script-only inside worker.

## Bus Schema

Likely additions:

```sql
CREATE TABLE runs (
  id INTEGER PRIMARY KEY,
  status TEXT NOT NULL,
  started_ts INTEGER NOT NULL,
  finished_ts INTEGER,
  config_hash TEXT,
  error TEXT
);

CREATE TABLE stage_items (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  stage TEXT NOT NULL,
  status TEXT NOT NULL,
  input_ref TEXT,
  claim_id TEXT,
  attempts INTEGER NOT NULL DEFAULT 0,
  result_meta TEXT,
  created_ts INTEGER NOT NULL,
  updated_ts INTEGER NOT NULL
);

CREATE TABLE stage_events (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  stage TEXT NOT NULL,
  level TEXT NOT NULL,
  message TEXT NOT NULL,
  meta TEXT,
  ts INTEGER NOT NULL
);

CREATE TABLE page_snapshots (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  platform TEXT NOT NULL,
  tab_url TEXT NOT NULL,
  fingerprint TEXT,
  snapshot_text TEXT,
  snapshot_ref TEXT,
  captured_ts INTEGER NOT NULL
);

CREATE TABLE job_seeds (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  platform TEXT NOT NULL,
  canonical_url TEXT NOT NULL,
  title TEXT,
  company TEXT,
  location TEXT,
  snippet TEXT,
  source_snapshot_id INTEGER REFERENCES page_snapshots(id),
  rank INTEGER,
  status TEXT NOT NULL,
  reason TEXT,
  UNIQUE(run_id, canonical_url)
);

CREATE TABLE detail_snapshots (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  seed_id INTEGER NOT NULL REFERENCES job_seeds(id),
  canonical_url TEXT NOT NULL,
  fetch_status TEXT NOT NULL,
  snapshot_text TEXT,
  snapshot_ref TEXT,
  error TEXT,
  captured_ts INTEGER NOT NULL
);
```

Keep existing `jobs`, `job_attempts`, `companies`, `meta`, but route writes
through stage-aware helpers.

## Script Rules

Scripts must run inside workers, not main.

Reason: command input/output becomes context of whichever agent ran it. If
main runs scripts, expensive main context grows with operational noise.

Script output contract:

- print compact receipts only
- never print HTML
- never print full JSONL rows
- never print report cards
- write bulk payloads to DB/temp files
- expose small `claim_next`, `claim_batch`, `finish`, `fail`, `summary`
  commands

DB concurrency:

- start serial
- add atomic claim scripts before parallel workers
- SQLite can handle this, but lock retries must happen in scripts, not chat

## Context Budget Rules

Main context:

- run id
- current phase
- compact worker receipts
- user choices

Worker context:

- prompt
- one claimed unit or small batch
- bounded config/prefs/snapshot reads
- compact script receipts

Payload caps:

- list snapshot: compact visible text, links, ranks
- detail snapshot: extracted visible job content only
- judge batch: target 5 jobs, shrink on large text
- evidence strings: short snippets only
- stdout: receipt only

Hard rule:

- if data is useful later, persist it
- if data is only operational noise, summarize it
- if data is large, never return it through chat

## Model Policy

Main supervisor:

- user/session controls it
- plugin should not depend on changing it

Subagents:

- set explicit `model` in frontmatter
- default to `haiku` for browser/ops/report
- default to `sonnet` for judge
- use `inherit` only for debug agents

Fallback:

- failed judge items can be retried with Sonnet if judge ever moves to Haiku
- Opus should be manual/debug only

## Parallelism Policy

Default:

- browser serial
- DB writes serial
- judge serial batches

Allowed later:

- parallel judge workers after atomic claim helpers
- parallel report is never useful
- parallel browser is probably not worth Chrome instability

Reason:

- speed is not king
- avoiding retries and context waste is king

## Command Shape

`/jessy:run` should:

1. invoke `jessy-ops-worker` prepare
2. loop `jessy-browser-worker` until browser queue done
3. loop `jessy-judge-worker` until judge queue done
4. invoke `jessy-report-worker` prepare
5. pause for user choice
6. invoke `jessy-report-worker` consume
7. invoke `jessy-learn-worker` only if cadence hit
8. invoke cleanup only if configured

Each loop iteration returns only a receipt.

## Migration Plan

1. Add DB tables and helper scripts for runs/stage queues/snapshots.
2. Convert current scan skill into `jessy-browser-worker` agent.
3. Move extraction + scoring into `jessy-judge-worker`.
4. Make scan command a supervisor flow, not a worker.
5. Move report scripts behind `jessy-report-worker`.
6. Ensure every helper has compact stdout tests.
7. Add smoke test: repeated run stops on history boundary and main transcript
   contains no job descriptions.

## Open Questions

- exact judge batch size after real snapshot token measurements
- whether detail snapshot should store compressed raw HTML debug refs
- whether learning should be its own agent or `ops-worker` mode
- whether scoring should produce both strict fields and free-form rationale
- how much old scan behavior to keep during phased migration
