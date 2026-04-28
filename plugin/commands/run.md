---
description: Run a full jessy pass — scan open LinkedIn tabs, then render the report. Requires `claude --chrome`.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-scan)
  - Skill(jessy-report)
---

Run jessy end-to-end.

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the **jessy-onboard** skill first and wait for it to finish.
2. Invoke the **jessy-scan** skill. Print its one-line summary.
3. Then invoke the **jessy-report** skill against current unseen rows.
4. Stop after report finishes — do not re-scan.
