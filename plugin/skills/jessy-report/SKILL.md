---
name: jessy-report
description: Render the ranked jessy job report from ~/.jessy/jessy.db, prompt the user to pick which jobs to open in Chrome, mark picks opened and the rest dismissed, and check the learning cadence. Use when the user runs /jessy:report or /jessy.
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/render_cards.sh*)
  - Bash(mktemp*)
  - Bash(grep *)
  - Bash(tail *)
  - Bash(rm -f *)
  - Bash(wc *)
  - Bash(tr *)
  - Bash(sed -n *)
  - Read
  - Skill(jessy-learn)
  - mcp__claude-in-chrome
  - mcp__claude-in-chrome__*
---

# jessy-report

Renders the report, captures the user's open/dismiss picks, persists the
choices, opens picked URLs in Chrome.

## Preconditions

- `~/.jessy/jessy.db` exists with rows where `user_action IS NULL`.
- For "Open in Chrome", a `claude --chrome` session is attached. If not,
  print picked URLs and tell the user to open them manually.
- On first Chrome-extension prompt, tell the user to allow access for the
  upcoming LinkedIn tab opens. Do not ask again unless Chrome prompts again.

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

### 3. Render visual + capture index map

Run render_cards.sh and capture stderr separately so the `INDEX_MAP` line
does not pollute the rendered output:

```
RENDER_ERR=$(mktemp)
printf '%s\n' "$JOBS" | \
  ${CLAUDE_PLUGIN_ROOT}/scripts/render_cards.sh \
    --match "$threshold_match" --low "$threshold_low_show" \
    2> "$RENDER_ERR"
INDEX_LINE=$(grep '^INDEX_MAP' "$RENDER_ERR" | tail -1)
rm -f "$RENDER_ERR"
```

Print stdout as-is. Do not reformat.

Parse `INDEX_LINE` by splitting on tabs: drop the leading `INDEX_MAP`
token; the remaining tokens are the candidate URLs in pick order (index
1..N). This is the index→URL map. If it is empty, there are no
pickable rows — skip step 4 and 5 (nothing to mark, nothing to open),
go to step 6.

### 4. Prompt for picks (numeric input)

Print exactly one line below the rendered output:

```
Open in Chrome — type indices (e.g. 1,3,5), 'all', or 'none':
```

Then stop and wait for the user's next chat message. Do **not** use
AskUserQuestion. The cards must remain visible in scrollback while the
user types.

Parse the user's reply (trim whitespace, lowercase for keyword check):

- empty or `none` → picks = [].
- `all` → picks = every URL in the index map.
- otherwise → split on commas and whitespace; for each token, parse as
  integer. Keep tokens that parse to an integer in `[1, N]`; dedupe
  preserving first-seen order. If any token fails to parse or is out
  of range, print one brief warning line listing the ignored tokens
  (e.g. `ignored: foo, 99`) and continue with the valid picks.

Map each pick index back to its URL via the index map.

### 5. Apply picks

For each picked URL:
- `db.sh mark_action <url> opened`
- Open in Chrome via the `claude --chrome` session (new tab per URL).
  If chrome not attached, collect them and print at the end:
  `open these manually:` followed by the URLs.

For every URL in the candidate index map that was not picked:
- `db.sh mark_action <url> dismissed`

For the ignored bucket (score < `threshold_low_show`, not in the index
map): also mark each as `dismissed`. Rule: each report fully consumes
its rows — "report shown = report done".

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
