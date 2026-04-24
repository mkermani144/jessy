---
description: Render the ranked report from ~/.jessy/jessy.db, prompt to open jobs in Chrome, mark picks/dismissals.
disable-model-invocation: true
---

Run the jessy report flow.

Invoke the `jessy-report` skill end-to-end. Print the rendered cards as-is,
ask the user which jobs to open, mark `user_action` on the chosen rows
(picked → opened, candidate-but-not-picked → dismissed; ignored bucket is
also marked dismissed per the skill's v1 rule). Then print the one-line
summary `opened N; dismissed M; unseen 0.`.
