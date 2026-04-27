---
description: Scan open LinkedIn job tabs in Chrome, score each against ~/.jessy/preferences.md, and persist new jobs. Requires `claude --chrome`.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-scan)
---

Run a jessy scan pass.

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the **jessy-onboard** skill first and wait for it to finish.
2. Invoke the **jessy-scan** skill end-to-end. Use the linkedin platform
   skill for page semantics.
3. When done, print the one-line summary
   (`scanned N new; M match; K low; L ignored`) and stop — do not also
   render the report. The user runs `/jessy:report` separately.
