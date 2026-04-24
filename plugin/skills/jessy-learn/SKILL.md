---
name: jessy-learn
description: Mine the user's recent open / dismiss decisions, propose lopsided patterns as candidate preference updates, and (with consent) rewrite ~/.jessy/preferences.md. Use when the report skill detects the learn cadence has been hit, or when the user runs /jessy:learn directly.
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Read
  - Edit
  - Write
  - AskUserQuestion
---

# jessy-learn

Heuristic learner. Reads ~50 recent acted-on jobs (opened + dismissed),
groups them by signal, surfaces lopsided clusters, and lets the user pick
which to add to `preferences.md`. Then resets the learn counter.

## Procedure

### 1. Read inputs

```
ROWS=$(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh recent_actions 50)
PREFS=$(cat ~/.jessy/preferences.md)
```

If `ROWS` is empty or fewer than ~10 acted-on rows total, print
`not enough signal yet (need ~10 acted-on jobs); skipping`. Reset cadence
to baseline anyway (so we don't keep re-checking immediately) and stop.

### 2. Cluster by signal

For each job in `ROWS`, you (Claude) extract the signals present:

| Signal kind   | Examples                                                    |
|---------------|-------------------------------------------------------------|
| stack         | `Java`, `Rust`, `Python`, `TypeScript`, `Go`, `C++`         |
| seniority     | `10+ yrs`, `senior`, `staff`, `principal`, `junior`, `intern` |
| domain        | `finance`, `crypto`, `adtech`, `ml`, `infra`, `gaming`      |
| location      | `remote EU`, `on-site Amsterdam`, `hybrid London`           |
| company size  | `1-10`, `11-50`, `51-200`, `201-1k`, `1k-10k`, `10k+`       |
| modality      | `easy apply`, `direct`, `recruiter-only`                    |

Read signals from `title`, `desc`, `req_hard`, `req_nice`, `company_size`,
`company_summary`. Be conservative — one clear signal per axis is fine,
don't invent.

### 3. Find candidate patterns

For each (signal_kind, signal_value) pair seen ≥ 3 times in ROWS,
compute opens vs dismisses. A **candidate pattern** has:

- Total occurrences ≥ 3
- One side (open or dismiss) ≥ 80% of occurrences
- The dominant side has count ≥ 3 (so a 1-of-3 doesn't trigger)

Direction → category:

| Dominant     | Strength               | Category in prefs.md |
|--------------|------------------------|----------------------|
| dismisses    | ratio ≥ 0.8 and N ≥ 3  | Dislikes             |
| dismisses    | ratio = 1.0 and N ≥ 5  | Dealbreakers (offer) |
| opens        | ratio ≥ 0.8 and N ≥ 3  | Likes                |

Format each candidate as a one-line label, e.g.:
- `Java roles → Dislikes (4 dismissed, 0 opened)`
- `remote EU → Likes (5 opened, 0 dismissed)`
- `finance domain → Dealbreakers (5 dismissed, 0 opened)`

If a candidate already exists in the matching `preferences.md` section
(case-insensitive substring), drop it from the candidate list.

If the candidate list is empty after filtering, print
`no new patterns; resetting cadence` and skip to step 6.

### 4. Confirm with user

`AskUserQuestion` (multiSelect: true). Question:
`Patterns I see — which to add to your prefs?`
Options: each candidate label. Plus an explicit `none of these` row that,
if picked, treats the whole answer as nothing-selected.

### 5. Apply picks

For each picked candidate:
- Read `~/.jessy/preferences.md`.
- Append a bullet under the right section (`## Dealbreakers`,
  `## Dislikes`, or `## Likes`). Bullet text = the signal value with a
  short qualifier, e.g. `- Java primary stack` or `- remote EU`.
- Preserve the section's existing bullets and any HTML comment placeholder
  (drop the placeholder only if the section was previously empty).
- Use Edit (preferred) or Write to persist.

After all picks applied, print:
`updated preferences.md (+N bullets)`.

### 6. Reset cadence

```
db.sh meta_set jobs_since_last_learn 0
idx=$(db.sh meta_get next_cadence_idx)
cadence_len=$(db.sh config_cadence | wc -l | tr -d ' ')

if [[ "$cadence_len" -eq 0 ]]; then
  # No cadence configured; leave idx untouched.
  :
else
  last=$(( cadence_len - 1 ))
  new_idx=$(( idx + 1 > last ? last : idx + 1 ))
  db.sh meta_set next_cadence_idx $new_idx
fi
```

`db.sh config_cadence` parses the inline `cadence: [N, M, ...]` array from
`~/.jessy/config.yaml` and emits one value per line. No `yq` dependency.

### 7. Final output

One line: `learn done — added N patterns; next check in M jobs`.
`M = db.sh config_cadence | sed -n "$((new_idx + 1))p"`. If cadence is
empty, print `learn done — added N patterns; no cadence configured`.

## What this skill does NOT do

- Modify thresholds in `config.yaml` — that's a manual user action.
- Re-score existing jobs.
- Touch jobs with `user_action IS NULL`.
- Invent patterns from < 3 examples.
