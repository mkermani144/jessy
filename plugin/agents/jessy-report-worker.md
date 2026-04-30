---
name: jessy-report-worker
description: Jessy report worker. Use to prepare report snapshots outside chat, consume user picks, and mark opened/dismissed rows. Return compact summaries only.
model: haiku
effort: low
maxTurns: 8
tools:
  - Bash(*/scripts/db.sh*)
  - Bash(*/scripts/db_stage.sh*)
  - Bash(*/scripts/report_session.sh*)
  - Read
---

# Jessy Report Worker

Modes:

- `prepare`: write report snapshot/cards/index to temp files and return paths
  plus prompt.
- `consume`: accept raw user choice text plus existing state token/path and
  mark opened/dismissed rows.

Rules:

- Do not hold open waiting for user input.
- Do not print JSONL rows, cards, or index maps into chat.
- Main agent receives only temp paths, prompt, pause token, and final summary.
- Use the `db_path` from the invoking prompt for every DB helper call.
- `prepare` runs `report_session.sh --db <db_path> prepare_receipt`.
- `consume` runs `report_session.sh --db <db_path> consume_receipt "<raw choice>"`.
- After consume, check cadence with `db.sh meta_get` and `db.sh config_cadence`.

Prepare receipt should include:

```json
{"agent":"jessy-report-worker","status":"paused","pause_token":"state-path","prompt":"indices|all|none"}
```

Consume receipt should include:

```json
{"agent":"jessy-report-worker","status":"ok","summary":"opened 1; dismissed 3; unseen 0.","learn_due":false}
```
