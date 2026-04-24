---
description: Show the path to ~/.jessy/config.yaml and tell the user to edit it. Runs onboarding first if ~/.jessy is missing.
disable-model-invocation: true
allowed-tools:
  - Bash(test *)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh*)
  - Read
  - AskUserQuestion
---

Expose the jessy config file for editing.

Steps:
1. If `~/.jessy/config.yaml` does not exist, invoke the onboarding flow:
   - Ask the user via `AskUserQuestion` for:
     (a) LinkedIn search URLs (free text, blank = none)
     (b) Dealbreakers (free text, blank = none)
     (c) Likes (free text, blank = none)
   - Write answers to temp files and call:
     `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh --non-interactive \
        --urls-file <path> --dealbreakers-file <path> --likes-file <path>`
2. Print the path `~/.jessy/config.yaml` and tell the user:
   `edit this in your editor, then re-run /jessy:config or any jessy command`.
3. Do NOT attempt to spawn `vi`, `vim`, `nano`, or any other editor via Bash.
   The Bash tool has no TTY and interactive editors will hang the session.
4. Offer to Read the file and show its contents if the user wants to review
   before editing. Do not modify it without an explicit Edit request.
