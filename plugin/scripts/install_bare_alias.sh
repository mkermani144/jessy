#!/usr/bin/env bash
# Install a user-level slash command at ~/.claude/commands/jessy.md
# so you can run `/jessy` (bare, no plugin namespace).
#
# Plugin commands are always namespaced as /<plugin>:<cmd>, so /jessy:run
# is the canonical alias. This script adds a side-car user command that
# delegates to the plugin skills, giving you `/jessy` for short.
#
# Idempotent: re-run is a no-op. Pass --force to overwrite.

set -euo pipefail

FORCE=0
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1 ;;
    -h|--help)
      cat <<EOF
usage: install_bare_alias.sh [--force]

Writes ~/.claude/commands/jessy.md — a user-level slash command that
runs the full jessy pass (scan + report). Requires the jessy plugin to
be loaded in the Claude Code session.
EOF
      exit 0
      ;;
    *) echo "install_bare_alias.sh: unknown arg: $arg" >&2; exit 2 ;;
  esac
done

DST="$HOME/.claude/commands/jessy.md"
mkdir -p "$(dirname "$DST")"

if [[ -e "$DST" && $FORCE -eq 0 ]]; then
  printf '%s already exists (use --force to overwrite)\n' "$DST" >&2
  exit 0
fi

cat > "$DST" <<'EOF'
---
description: Full jessy pass — scan open LinkedIn tabs, then render report. Requires the jessy plugin to be loaded.
disable-model-invocation: true
allowed-tools:
  - Skill(jessy-onboard)
  - Skill(jessy-scan)
  - Skill(jessy-report)
---

Run a full jessy pass:

1. If `~/.jessy/config.yaml` or `~/.jessy/preferences.md` is missing,
   invoke the `jessy-onboard` skill first and wait for it to finish.
2. Invoke the `jessy-scan` skill. Print its one-line summary.
3. Invoke the `jessy-report` skill against the rows just inserted.

If the jessy plugin is not loaded, tell the user to relaunch with
`claude --settings /path/to/jessy/plugin/.claude/settings.json --plugin-dir /path/to/jessy/plugin --chrome`
and stop.

Stop after report finishes — do not re-scan.
EOF

printf 'wrote %s\n' "$DST" >&2
printf 'run /reload-plugins (or restart Claude Code) to pick up /jessy\n' >&2
