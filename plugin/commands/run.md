---
description: Run a full context-isolated jessy pass. Requires `claude --chrome`.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-scan)
  - Skill(jessy-report)
---

Run jessy end-to-end.

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the **jessy-onboard** skill first and wait for it to finish.
2. Invoke **jessy-scan**. It supervises ops/browser/judge workers and prints
   only a one-line scan summary. If the user supplied an alternate DB path,
   pass it as `db_path` and require workers to use `--db <db_path>`.
3. Invoke **jessy-report**. It prepares report artifacts outside chat, pauses
   for the user's choices, then consumes them through the report worker.
4. Stop after report finishes. Do not re-scan.
