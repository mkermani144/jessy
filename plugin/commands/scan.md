---
description: Scan open LinkedIn / Wellfound tabs through receipt-only workers. Requires `claude --chrome`.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-scan)
---

Run a jessy scan pass.

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the **jessy-onboard** skill first and wait for it to finish.
2. Invoke the **jessy-scan** skill end-to-end. It must keep job payloads in
   DB/temp files and return only worker receipts to the supervisor.
3. When done, print the one-line summary
   (`scanned N new; M match; K low; L ignored`, optionally `; cap hit`)
   and stop — do not also render the report. The user runs
   `/jessy:report` separately.
