---
name: jessy-report
description: Render the ranked jessy job report from ~/.jessy/jessy.db, prompt the user to pick which jobs to open in Chrome, mark picks opened and the rest dismissed, and check the learning cadence. Use when the user runs /jessy:report or /jessy.
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/render_cards.sh*)
  - Bash(test *)
  - Read
  - AskUserQuestion
---

# jessy-report

Renders the report, captures the user's open/dismiss picks, persists the
choices, opens picked URLs in Chrome.

## Preconditions

- `~/.jessy/jessy.db` exists with rows where `user_action IS NULL`.
- For "Open in Chrome", a `claude --chrome` session is attached. If not,
  print picked URLs and tell the user to open them manually.

## Procedure

### 1. Read config

`~/.jessy/config.yaml` → `threshold_match`, `threshold_low_show`,
`learning.cadence`.

### 2. Pull report rows

```
JOBS=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh query_report)
```

`JOBS` is JSONL, one job per line, sorted by score DESC. Each line has:
`url, title, desc, req_hard, req_nice, platform, score, rationale,
user_action, ts, company_name, company_size, company_summary`.

If `JOBS` is empty, print `no unseen jobs — run /jessy:scan first` and stop.

### 3. Render visual

```
printf '%s\n' "$JOBS" | \
  ${CLAUDE_PLUGIN_ROOT}/scripts/render_cards.sh \
    --match "$threshold_match" --low "$threshold_low_show"
```

Print the result as-is. Do not reformat.

### 4. Build the multiselect

From the same `JOBS`, take rows where `score >= threshold_low_show`
(i.e. match + low buckets — ignored bucket is excluded from picks).

Construct one option per row:
- `label`: `"{title} @ {company_name} (score {score})"` truncated to ~80 chars.
- `value`: the canonical `url`.

Use `AskUserQuestion` with `multiSelect: true`. Question:
`Open in Chrome (multi-select; leave empty to dismiss all):`.

If the candidate list is empty, skip the prompt.

### 5. Apply picks

For each picked URL:
- `db.sh mark_action <url> opened`
- Open in Chrome via the `claude --chrome` session (new tab per URL).
  If chrome not attached, collect them and print at the end:
  `open these manually:` followed by the URLs.

For each unpicked URL in the candidate list (match + low minus picks):
- `db.sh mark_action <url> dismissed`

Ignored bucket (score < `threshold_low_show`) is left unmarked — they stay
`user_action IS NULL` so a later report can revisit them if thresholds change.

Wait — that contradicts the "fresh report = unseen" model. Decision for v1:
**also mark ignored as dismissed** (so each report fully consumes its rows).
Easier to reason about, matches user expectation that "report shown = report
done". Track this as a future open question if thresholds become dynamic.

### 6. Check learn cadence

```
since=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh meta_get jobs_since_last_learn || echo 0)
idx=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh meta_get next_cadence_idx || echo 0)
cadence_len=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh config_cadence | wc -l | tr -d ' ')
```

If `cadence_len == 0`, cadence is disabled; skip this step silently.

Otherwise clamp `idx` to `[0, cadence_len - 1]` and fetch the target:
```
target=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh config_cadence | sed -n "$((idx + 1))p")
```

If `since >= target`, print:
```
↳ learn threshold hit (since={since}, target={target}). Running jessy-learn…
```
Then invoke the **jessy-learn** skill in this same turn. Its output prints
inline. Continue with step 7 after it returns.

If the threshold is not hit, skip silently.

### 7. Final summary

Print one line:
`opened {N}; dismissed {M}; unseen {0}.`

## What this skill does NOT do

- Re-score jobs. Scoring happened during scan.
- Modify `companies` rows.
- Open tabs without explicit user pick.
- Auto-apply.
