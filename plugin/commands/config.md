---
description: Show the path to ~/.jessy/config.yaml for the user to edit. Runs onboarding first if ~/.jessy is missing.
disable-model-invocation: true
allowed-tools:
  - Bash(test *)
  - Read
---

Expose the jessy config file for editing.

Steps:

1. If `~/.jessy/config.yaml` does not exist, invoke the **jessy-onboard**
   skill first and wait for it to finish.
2. Print the path `~/.jessy/config.yaml` and tell the user:
   `edit this in your editor, then re-run /jessy:config or any jessy command`.
3. Do NOT attempt to spawn `vi`, `vim`, `nano`, or any other editor via
   Bash. The Bash tool has no TTY and interactive editors will hang.
4. Offer to Read the file and show its contents if the user asks.
