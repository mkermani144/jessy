#!/usr/bin/env bash
# Verify onboarding writes platform-scoped startup URLs.

set -euo pipefail

command -v jq >/dev/null 2>&1 || { echo "skip: jq missing" >&2; exit 0; }
command -v sqlite3 >/dev/null 2>&1 || { echo "skip: sqlite3 missing" >&2; exit 0; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ONBOARD="$ROOT/plugin/scripts/onboard.sh"

TMP_HOME="$(mktemp -d)"
LI_URLS="$(mktemp)"
WF_URLS="$(mktemp)"
DEALBREAKERS="$(mktemp)"
LIKES="$(mktemp)"
trap 'rm -rf "$TMP_HOME"; rm -f "$LI_URLS" "$WF_URLS" "$DEALBREAKERS" "$LIKES"' EXIT

printf '%s\n%s\n' \
  'https://www.linkedin.com/jobs/search/?keywords=rust' \
  'https://example.com/not-jobs' > "$LI_URLS"
printf '%s\n%s\n' \
  'https://wellfound.com/role/software-engineer' \
  'https://example.com/not-jobs' > "$WF_URLS"
printf 'contract\n' > "$DEALBREAKERS"
printf 'rust\n' > "$LIKES"

HOME="$TMP_HOME" "$ONBOARD" \
  --non-interactive \
  --urls-file "$LI_URLS" \
  --wellfound-urls-file "$WF_URLS" \
  --dealbreakers-file "$DEALBREAKERS" \
  --likes-file "$LIKES" >/dev/null

CONFIG="$TMP_HOME/.jessy/config.yaml"
PREFS="$TMP_HOME/.jessy/preferences.md"

grep -Fq 'platforms:' "$CONFIG"
grep -Fq 'skip_title_keywords: []' "$CONFIG"
grep -Fq '  linkedin:' "$CONFIG"
grep -Fq '  wellfound:' "$CONFIG"
grep -Fq '      - https://www.linkedin.com/jobs/search/?keywords=rust' "$CONFIG"
grep -Fq '      - https://wellfound.com/role/software-engineer' "$CONFIG"
! grep -Fq 'https://example.com/not-jobs' "$CONFIG"
grep -Fq -- '- contract' "$PREFS"
grep -Fq -- '- rust' "$PREFS"
