---
description: Scan open LinkedIn job tabs in Chrome, score each against ~/.jessy/preferences.md, and persist new jobs. Requires `claude --chrome`.
disable-model-invocation: true
---

Run a jessy scan pass.

Invoke the `jessy-scan` skill end-to-end. Use the linkedin platform skill
for page semantics. When done, print the one-line summary
(`scanned N new; M match; K low; L ignored`) and stop — do not also render
the report. The user runs `/jessy:report` separately.

If `~/.jessy/config.yaml` is missing, run
`${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh` first.
