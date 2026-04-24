---
description: Render the ranked report from ~/.jessy/jessy.db, prompt to open jobs in Chrome, mark picks/dismissals.
disable-model-invocation: true
---

Run the jessy report flow.

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the **jessy-onboard** skill first and wait for it to finish.
2. Invoke the **jessy-report** skill end-to-end. Print the rendered cards
   as-is. Ask the user which jobs to open. Mark `user_action` on picked
   and candidate-but-not-picked rows (ignored bucket is also marked
   dismissed per the skill's v1 rule).
3. Print the one-line summary `opened N; dismissed M; unseen 0.`.
