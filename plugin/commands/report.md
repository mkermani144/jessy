---
description: Prepare the ranked report outside chat, prompt for picks, mark picks/dismissals.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-report)
---

Run the jessy report flow.

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the **jessy-onboard** skill first and wait for it to finish.
2. Invoke the **jessy-report** skill end-to-end. It writes the report to
   temp files and opens it in `tmux`/`less` when available; do not print
   rendered cards, JSONL, or index maps into chat.
3. Ask the user for indices, `all`, or `none`. Mark `user_action` on picked
   rows as `opened` and all other snapshot rows as `dismissed`.
4. Print the one-line summary `opened N; dismissed M; unseen 0.`.
