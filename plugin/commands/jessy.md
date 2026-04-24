---
description: Run a full jessy pass — scan open LinkedIn tabs, then render the report. Requires `claude --chrome`.
disable-model-invocation: true
---

Run jessy end-to-end:

1. Invoke the `jessy-scan` skill. Print its summary line.
2. Then invoke the `jessy-report` skill against the rows just inserted.

If `~/.jessy/config.yaml` is missing, run
`${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh` before step 1.

Stop after report finishes — do not re-scan.
