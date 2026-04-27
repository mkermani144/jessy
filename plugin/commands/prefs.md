---
description: Show the path to ~/.jessy/preferences.md for the user to edit. Runs onboarding first if ~/.jessy is missing.
disable-model-invocation: true
allowed-tools:
  - Bash(test *)
  - Skill(jessy-onboard)
  - Read
---

Expose the jessy preferences file for editing.

Steps:

1. If `~/.jessy/preferences.md` does not exist, invoke the **jessy-onboard**
   skill first and wait for it to finish.
2. Print the path `~/.jessy/preferences.md` and tell the user:
   `edit this in your editor, then re-run /jessy:prefs or any jessy command`.
3. Do NOT spawn an editor from Bash (no TTY → hangs).
4. Offer to Read the file if the user asks.
