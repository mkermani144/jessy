#!/usr/bin/env bash
# Verify report_session keeps bulky report output in files and consumes picks.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB="$ROOT/plugin/scripts/db.sh"
REPORT="$ROOT/plugin/scripts/report_session.sh"
TMP_DB="$(mktemp)"
TMP_STATE="$(mktemp)"
trap 'rm -f "$TMP_DB" "$TMP_STATE"' EXIT

export JESSY_DB="$TMP_DB"
export JESSY_REPORT_STATE="$TMP_STATE"
export JESSY_REPORT_NO_TMUX=1
export JESSY_REPORT_WIDTH=64

setup_db() {
  rm -f "$TMP_DB"
  "$DB" init
  local cid
  cid="$("$DB" upsert_company Acme "50" "Builds tools")"
  "$DB" insert_job "https://jobs.example.com/match" "$cid" "Match Job" \
    "Match desc" '["Rust"]' '[]' linkedin 91 "strong" >/dev/null
  "$DB" insert_job "https://jobs.example.com/low" "$cid" "Low Job" \
    "Low desc" '["Go"]' '[]' linkedin 52 "maybe" >/dev/null
  "$DB" insert_job "https://jobs.example.com/ignored" "$cid" "Ignored Job" \
    "Ignored desc" '[]' '[]' linkedin 12 "weak" >/dev/null
}

field_from_prepare() {
  local key="$1" text="$2"
  sed -n "s/^${key}=//p" <<< "$text"
}

assert_action() {
  local url="$1" action="$2" got
  got="$(sqlite3 "$TMP_DB" "SELECT user_action FROM jobs WHERE url = '$url';")"
  [[ "$got" == "$action" ]] || {
    printf 'expected %s -> %s, got %s\n' "$url" "$action" "$got" >&2
    exit 1
  }
}

setup_db

prepare_out="$("$REPORT" prepare)"
snapshot="$(field_from_prepare snapshot "$prepare_out")"
cards="$(field_from_prepare cards "$prepare_out")"
index="$(field_from_prepare index_map "$prepare_out")"

grep -q '^snapshot=' <<< "$prepare_out"
grep -q '^cards=' <<< "$prepare_out"
grep -q '^index_map=' <<< "$prepare_out"
grep -q '^state=' <<< "$prepare_out"
grep -q 'Review report outside chat' <<< "$prepare_out"
! grep -q 'Match Job' <<< "$prepare_out"
! grep -q 'https://jobs.example.com' <<< "$prepare_out"

[[ -s "$snapshot" ]]
[[ -s "$cards" ]]
[[ -s "$index" ]]
perl -Mutf8 -Mopen=:std,:encoding\(UTF-8\) -ne 'chomp; die qq(line $. too wide: $_\n) if length($_) > 64' "$cards"
grep -q $'^1\thttps://jobs.example.com/match$' "$index"
grep -q $'^2\thttps://jobs.example.com/low$' "$index"
! grep -q 'ignored' "$index"

summary="$("$REPORT" consume '2,99,foo,2')"
[[ "$summary" == "opened 1; dismissed 2; unseen 0." ]]
assert_action "https://jobs.example.com/match" dismissed
assert_action "https://jobs.example.com/low" opened
assert_action "https://jobs.example.com/ignored" dismissed

setup_db
"$REPORT" prepare >/dev/null
summary="$("$REPORT" consume all)"
[[ "$summary" == "opened 2; dismissed 1; unseen 0." ]]
assert_action "https://jobs.example.com/match" opened
assert_action "https://jobs.example.com/low" opened
assert_action "https://jobs.example.com/ignored" dismissed

setup_db
"$REPORT" prepare >/dev/null
summary="$("$REPORT" consume none)"
[[ "$summary" == "opened 0; dismissed 3; unseen 0." ]]
