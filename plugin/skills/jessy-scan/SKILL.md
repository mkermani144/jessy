---
name: jessy-scan
description: Supervise context-isolated Jessy browser and judge workers. Main thread sees receipts only, never job payloads.
model: haiku
effort: low
user-invocable: false
allowed-tools:
  - Skill(jessy-onboard)
  - Agent
  - mcp__claude-in-chrome
  - mcp__claude-in-chrome__*
---

# jessy-scan

Main agent is supervisor only. It never reads job descriptions, HTML, report
cards, extracted JSON, snapshot rows, or script row dumps.

## Preconditions

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing, invoke
   `jessy-onboard`, then continue.
2. Chrome session is attached (`claude --chrome`).
3. User is signed into enabled platforms.

## Flow

1. Invoke `jessy-ops-worker` to initialize DB, read small config flags, create
   a `runs` row, and enqueue browser work. It returns
   `{run_id,next,status,db_path}`.
2. Loop `jessy-browser-worker` serially until its receipt says `done:true`.
   Browser worker owns Chrome, history/title prefilter, `page_snapshots`,
   `job_seeds`, `detail_snapshots`, and browser-stage item status.
3. Loop `jessy-judge-worker` serially until its receipt says `done:true`.
   Judge worker owns preferences, bounded detail reads, extraction JSON,
   scoring, `jobs`, and `job_attempts`.
4. Invoke `jessy-ops-worker` to finish the run and optionally report cleanup
   threshold/cadence metadata.

## Worker Contracts

Use Agent with these custom agent names:

- `jessy-ops-worker`
- `jessy-browser-worker`
- `jessy-judge-worker`

Pass `run_id` and `db_path` to every worker invocation. Workers must call DB
helpers with the explicit global flag:

```text
${CLAUDE_PLUGIN_ROOT}/scripts/db_stage.sh --db <db_path> ...
${CLAUDE_PLUGIN_ROOT}/scripts/db_scan.sh --db <db_path> ...
${CLAUDE_PLUGIN_ROOT}/scripts/db.sh --db <db_path> ...
```

Each worker response must be a compact receipt. Acceptable examples:

```json
{"agent":"jessy-browser-worker","status":"ok","run_id":42,"claimed":1,"wrote":8,"failed":0,"done":false}
```

```json
{"agent":"jessy-judge-worker","status":"ok","run_id":42,"claimed":5,"wrote":5,"failed":0,"done":true}
```

If a worker returns payload text, stop and ask it to persist the payload and
return a receipt instead.

## Output

Print one final summary only:

```text
scanned N new; M match; K low; L ignored
```

Append `; cap hit` if any browser receipt reports a cap hit. Print optional
timing only if workers return compact timing fields.

## Forbidden

- No main-thread scoring.
- No main-thread DB row dumps.
- No report rendering.
- No parallel browser workers.
- Parallel judge workers are allowed only through `db_stage.sh claim_batch`;
  start with serial batches and enable parallelism only after local claim
  tests pass.
