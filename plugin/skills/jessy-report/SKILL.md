---
name: jessy-report
description: Prepare the ranked jessy job report outside chat, prompt for picks, mark picked jobs opened and the rest dismissed, and check the learning cadence. Use when the user runs /jessy:report or /jessy.
model: haiku
effort: low
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/report_session.sh*)
  - Read
  - Skill(jessy-learn)
---

# jessy-report

Prepares report artifacts outside chat, captures the user's pick reply, and
persists opened/dismissed choices.

## Preconditions

- `~/.jessy/jessy.db` exists.
- `tmux` + `less` are optional. If available in the current session,
  `report_session.sh prepare` opens the rendered report in a tmux window.

## Procedure

### 1. Prepare outside-chat report

Run:

```
${CLAUDE_PLUGIN_ROOT}/scripts/report_session.sh prepare
```

Print its stdout as-is. It contains only temp paths plus one prompt.
It must not print report JSONL, rendered cards, or the full index map
into chat.

If the prompt says `No unseen jobs; run /jessy:scan first.`, stop.

### 2. Wait for picks

Stop and wait for the user's next chat message. Do **not** use
AskUserQuestion.

Accepted replies:

- empty or `none`
- `all`
- comma/space-separated indices, e.g. `1,3,5`

### 3. Consume prepared snapshot

Run:

```
${CLAUDE_PLUGIN_ROOT}/scripts/report_session.sh consume "<user reply>"
```

Print its stdout as-is. It is the final consume summary only:
`opened N; dismissed M; unseen 0.`

The helper reads the prepared temp JSONL snapshot and index TSV. It maps
indices to URLs internally, calls `db.sh consume_report`, marks picked
snapshot URLs `opened`, and marks every other snapshot URL `dismissed`,
including ignored rows not present in the pickable index.

### 4. Check learn cadence

Call `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh meta_get jobs_since_last_learn`,
`${CLAUDE_PLUGIN_ROOT}/scripts/db.sh meta_get next_cadence_idx`, and
`${CLAUDE_PLUGIN_ROOT}/scripts/db.sh config_cadence`. Treat empty meta
values as `0`. Count cadence lines yourself.

If `cadence_len == 0`, cadence is disabled; skip this step silently.

Otherwise clamp `idx` to `[0, cadence_len - 1]` and select the target
from the cadence lines by zero-based index.

If `since >= target`, print:
```
↳ learn threshold hit (since={since}, target={target}). Running jessy-learn…
```
Then invoke the **jessy-learn** skill in this same turn. Its output prints
inline.

If the threshold is not hit, skip silently.

## What this skill does NOT do

- Re-score jobs. Scoring happened during scan.
- Modify `companies` rows.
- Print report JSONL/cards/index map into chat.
- Auto-apply.
