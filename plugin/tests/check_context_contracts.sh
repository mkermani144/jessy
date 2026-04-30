#!/usr/bin/env bash
# Static guardrails for context-isolated supervisor skills.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCAN="$ROOT/plugin/skills/jessy-scan/SKILL.md"
REPORT="$ROOT/plugin/skills/jessy-report/SKILL.md"

grep -q 'jessy-browser-worker' "$SCAN"
grep -q 'jessy-judge-worker' "$SCAN"
grep -q 'No main-thread scoring' "$SCAN"
grep -q 'No main-thread `report_session.sh`' "$REPORT"

if grep -q 'Main thread owns matching' "$SCAN"; then
  echo "scan skill reintroduced main-thread scoring ownership" >&2
  exit 1
fi

if grep -q 'No per-card judge subagents' "$SCAN"; then
  echo "scan skill reintroduced old no-judge-subagent rule" >&2
  exit 1
fi
