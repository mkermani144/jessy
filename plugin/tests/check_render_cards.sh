#!/usr/bin/env bash
# Fixture checks for report card wrapping and compact low/ignored rows.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RENDER="$ROOT/plugin/scripts/render_cards.sh"
FIXTURE="$ROOT/plugin/tests/render_cards_fixture.jsonl"

check_width() {
  local width="$1"
  local out clean err
  out="$(mktemp)"
  clean="$(mktemp)"
  err="$(mktemp)"

  "$RENDER" --width "$width" < "$FIXTURE" > "$out" 2> "$err"
  perl -pe 's/\e\[[0-9;]*m//g' "$out" > "$clean"

  grep -q $'\033\\[38;5;255m' "$out"
  grep -q $'\033\\[38;5;250m' "$out"
  perl -Mutf8 -Mopen=:std,:encoding\(UTF-8\) -ne "chomp; die qq(line $. exceeds $width chars: \$_\n) if length(\$_) > $width" "$clean"
  grep -q 'Status:' "$clean"
  grep -q 'Url:' "$clean"
  grep -q 'Lang:' "$clean"
  grep -q 'Title:' "$clean"
  grep -q 'Summary:' "$clean"
  grep -q 'Req:' "$clean"
  grep -q 'Evidence:' "$clean"
  grep -q 'Company:' "$clean"
  grep -q 'Company Size:' "$clean"
  grep -q 'Visa:' "$clean"
  grep -q 'Principal Platform Engineer' "$clean"
  grep -q 'Northstar Commerce Infrastructure' "$clean"
  grep -q 'ok' "$clean"
  grep -q '1200-1500' "$clean"
  grep -q 'remote US' "$clean"
  grep -q '\$180k-\$220k' "$clean"
  grep -q 'Remote - United States' "$clean"
  grep -q 'Incident' "$clean"
  grep -q 'postmortem' "$clean"
  grep -q 'https://jobs.example.com/northstar' "$clean"
  grep -q '^\[2\] low 52:' "$clean"
  grep -q 'stack differs' "$clean"
  grep -q '^+1 more non-match jobs ignored$' "$clean"
  grep -q $'^INDEX_MAP\thttps://jobs.example.com/northstar' "$err"

  rm -f "$out" "$clean" "$err"
}

check_width 56
check_width 80
