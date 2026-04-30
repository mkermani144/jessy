---
name: jessy-report
description: Supervise the report worker. Prepare report outside chat, pause for picks, consume choices, then check learning cadence by worker receipt.
model: haiku
effort: low
user-invocable: false
allowed-tools:
  - Agent
  - Skill(jessy-learn)
---

# jessy-report

Main agent sees no report rows, cards, or index maps.

## Flow

1. Invoke `jessy-report-worker` in `prepare` mode.
2. Print only the worker prompt and artifact paths/pause token.
3. Stop and wait for the user's next chat message. Do not use AskUserQuestion.
4. Invoke `jessy-report-worker` in `consume` mode with only:
   - raw user reply
   - pause token/state path from prepare
5. Print its compact final summary:
   `opened N; dismissed M; unseen 0.`
6. If the worker receipt says learning cadence hit, invoke `jessy-learn`.

## Accepted Replies

- empty or `none`
- `all`
- comma/space-separated indices, e.g. `1,3,5`

## Forbidden

- No main-thread `report_session.sh`.
- No main-thread `db.sh query_report`.
- No JSONL/cards/index-map chat output.
- No auto-apply learning edits without explicit consent.
