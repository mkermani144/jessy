---
description: Prepare the ranked report through the report worker, prompt for picks, mark picks/dismissals.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-report)
---

Run the jessy report flow.

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the **jessy-onboard** skill first and wait for it to finish.
2. Invoke the **jessy-report** skill end-to-end. It delegates prepare/consume
   to `jessy-report-worker`; main chat sees only paths, prompt, and summary.
3. Ask the user for indices, `all`, or `none`. The worker marks picked rows
   as `opened` and all other snapshot rows as `dismissed`.
4. Print the one-line summary `opened N; dismissed M; unseen 0.`.
