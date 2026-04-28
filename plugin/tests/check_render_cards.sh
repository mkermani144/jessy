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
  grep -q 'Status:' "$out"
  grep -q 'Url:' "$out"
  grep -q 'Lang:' "$out"
  grep -q 'Title:' "$out"
  grep -q 'Summary:' "$out"
  grep -q 'Req:' "$out"
  grep -q 'Evidence:' "$out"
  grep -q 'Company:' "$out"
  grep -q 'Company Size:' "$out"
  grep -q 'Visa:' "$out"
  grep -q 'Principal Platform Engineer' "$out"
  grep -q 'Northstar Commerce Infrastructure' "$out"
  grep -q 'ok' "$out"
  grep -q '1200-1500' "$out"
  grep -q 'remote US' "$out"
  grep -q '\$180k-\$220k' "$out"
  grep -q 'Remote - United States' "$out"
  grep -q 'Incident' "$out"
  grep -q 'postmortem' "$out"
  grep -q 'https://jobs.example.com/northstar' "$out"
  grep -q '^\[2\] low 52:' "$out"
  grep -q 'stack differs' "$out"
  grep -q '^+1 more non-match jobs ignored$' "$out"
  grep -q $'^INDEX_MAP\thttps://jobs.example.com/northstar' "$err"

  rm -f "$out" "$err"
}

check_width 56
check_width 80
