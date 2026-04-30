---
name: jessy-cleanup
description: Internal cleanup stage. Prune old / acted-on rows from ~/.jessy/jessy.db using config.yaml limits.
user-invocable: false
allowed-tools:
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/db.sh*)
  - Read
---

# jessy-cleanup

Thin wrapper around `${CLAUDE_PLUGIN_ROOT}/scripts/db.sh cleanup`. Reads
`cleanup` section from `~/.jessy/config.yaml`:

- `cleanup.max_age_days`
- `cleanup.max_rows`

Then runs:

```
${CLAUDE_PLUGIN_ROOT}/scripts/db.sh cleanup <max_age_days> <max_rows>
```

Print the script's `pruned X; now Y rows` output as-is.

## Safety

`${CLAUDE_PLUGIN_ROOT}/scripts/db.sh cleanup` only deletes rows where
`user_action IS NOT NULL`.
Rows the user has not yet seen (`user_action IS NULL`) are never
touched, regardless of age or row count. No confirmation prompt
needed — the operation is bounded by config and excludes unseen rows.

## Errors

If `~/.jessy/config.yaml` is missing the `cleanup` section, fall back to
defaults: `max_age_days=30`, `max_rows=5000`.
