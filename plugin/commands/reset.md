---
description: Wipe ~/.jessy (config, preferences, DB) and re-run first-run onboarding from scratch. Destructive — confirms first.
disable-model-invocation: true
allowed-tools:
  - Bash(rm -rf ~/.jessy)
  - Skill(jessy-onboard)
  - AskUserQuestion
---

Full reset of jessy local state.

Steps:

1. **Confirm** via AskUserQuestion:
   `This deletes ~/.jessy (config.yaml, preferences.md, jessy.db). No backup. Proceed?`
   Options: `yes, purge` / `cancel`. If anything other than `yes, purge`, stop and print `cancelled`.
2. Run `rm -rf ~/.jessy`. The directory and every file inside (config,
   prefs, DB, any prior `backup-*` subdirs) are gone after this — no
   undo.
3. Invoke the **jessy-onboard** skill. It will see `~/.jessy` missing and
   walk the user through fresh URLs / dealbreakers / likes, then write
   new `config.yaml`, `preferences.md`, and an empty `jessy.db`.
4. Print `reset done — ~/.jessy rebuilt from scratch`.

## Notes

- Unlike `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh --force` (which backs
  up to `~/.jessy/backup-<ts>/`),
  this command takes no backup. Past state is unrecoverable.
- Safe to run when `~/.jessy` already does not exist — `rm -rf` is a
  no-op, then onboarding runs normally.
