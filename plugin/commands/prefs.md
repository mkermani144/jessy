---
description: Show the path to ~/.jessy/preferences.md and tell the user to edit it. Runs onboarding first if ~/.jessy is missing.
disable-model-invocation: true
allowed-tools:
  - Bash(test *)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh*)
  - Read
  - AskUserQuestion
---

Expose the jessy preferences file for editing.

Steps:
1. If `~/.jessy/preferences.md` does not exist, invoke the onboarding flow:
   - Ask the user via `AskUserQuestion` for:
     (a) LinkedIn search URLs
     (b) Dealbreakers
     (c) Likes
   - Write answers to temp files and call:
     `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh --non-interactive \
        --urls-file <path> --dealbreakers-file <path> --likes-file <path>`
2. Print the path `~/.jessy/preferences.md` and tell the user:
   `edit this in your editor, then re-run /jessy:prefs or any jessy command`.
3. Do NOT spawn `vi`, `vim`, `nano`, or any other editor via Bash.
   The Bash tool has no TTY and interactive editors will hang.
4. Offer to Read the file. Do not modify it without an explicit Edit request.
