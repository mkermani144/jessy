---
description: Prune old / acted-on rows from ~/.jessy/jessy.db using config limits. Never touches unseen rows.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-cleanup)
---

1. If `~/.jessy/config.yaml` is missing, invoke the **jessy-onboard**
   skill first and wait for it to finish.
2. Invoke the **jessy-cleanup** skill. Print the `pruned X; now Y rows`
   summary.
