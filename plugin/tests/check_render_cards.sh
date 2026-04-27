#!/usr/bin/env bash
# Fixture checks for report card wrapping and compact low/ignored rows.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RENDER="$ROOT/plugin/scripts/render_cards.sh"
FIXTURE="$ROOT/plugin/tests/render_cards_fixture.jsonl"

check_width() {
  local width="$1"
  local out err
  out="$(mktemp)"
  err="$(mktemp)"

  "$RENDER" --width "$width" < "$FIXTURE" > "$out" 2> "$err"

  perl -Mutf8 -Mopen=:std,:encoding\(UTF-8\) -ne "chomp; die qq(line $. exceeds $width chars: \$_\n) if length(\$_) > $width" "$out"
  grep -q 'Summary:' "$out"
  grep -q 'Must:' "$out"
  grep -q 'Company:' "$out"
  grep -q 'Link:' "$out"
  grep -q 'Principal Platform Engineer' "$out"
  grep -q 'Northstar Commerce Infrastructure' "$out"
  grep -q 'Incident' "$out"
  grep -q 'postmortem' "$out"
  grep -q 'https://jobs.example.com/northstar' "$out"
  grep -q '^\[2\] low 52:' "$out"
  grep -q '^+1 more non-match jobs ignored$' "$out"
  grep -q $'^INDEX_MAP\thttps://jobs.example.com/northstar' "$err"

  rm -f "$out" "$err"
}

check_width 56
check_width 80
