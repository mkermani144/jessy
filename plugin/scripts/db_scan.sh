#!/usr/bin/env bash
# Scan-only DB helpers. Keeps Claude Bash calls as one allowlisted command.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_SH="$SCRIPT_DIR/db.sh"

usage() {
  cat >&2 <<'EOF'
usage: db_scan.sh <subcommand> [args...]

subcommands:
  attempted_many <url...>
    print "<url>\t<yes|no>" for each URL
  skip_job <url> <company> <title> <desc> <score> <rationale>
    start attempt, insert ignored job row, finish scored
  score_job <url> <company> <company_size> <title> <desc> \
            <req_json> <nice_json> <score> <rationale> [extract_json]
    start attempt, insert scored job row, finish scored with extraction JSON
  fail_attempt <url> <reason> [extract_json]
    start attempt, finish failed
  bump_learn <delta>
    add delta to meta jobs_since_last_learn
EOF
  exit 2
}

cmd_attempted_many() {
  [[ $# -gt 0 ]] || usage
  local url
  for url in "$@"; do
    printf '%s\t%s\n' "$url" "$("$DB_SH" attempted "$url")"
  done
}

cmd_skip_job() {
  [[ $# -eq 6 ]] || usage
  local url="$1" company="$2" title="$3" desc="$4" score="$5" rationale="$6"
  "$DB_SH" attempt_start "$url" linkedin >/dev/null
  local cid
  cid="$("$DB_SH" upsert_company "$company" "" "")"
  "$DB_SH" insert_job "$url" "$cid" "$title" "$desc" '[]' '[]' linkedin "$score" "$rationale"
  "$DB_SH" attempt_finish "$url" scored "" "$score" "$rationale" >/dev/null
}

cmd_score_job() {
  [[ $# -ge 9 ]] || usage
  local url="$1" company="$2" size="$3" title="$4" desc="$5"
  local req="$6" nice="$7" score="$8" rationale="$9" extract_json="${10:-}"
  "$DB_SH" attempt_start "$url" linkedin >/dev/null
  local cid
  cid="$("$DB_SH" upsert_company "$company" "$size" "")"
  "$DB_SH" insert_job "$url" "$cid" "$title" "$desc" "$req" "$nice" linkedin "$score" "$rationale"
  "$DB_SH" attempt_finish "$url" scored "$extract_json" "$score" "$rationale" >/dev/null
}

cmd_fail_attempt() {
  [[ $# -ge 2 ]] || usage
  local url="$1" reason="$2" extract_json="${3:-}"
  "$DB_SH" attempt_start "$url" linkedin >/dev/null
  "$DB_SH" attempt_finish "$url" failed "$extract_json" "" "$reason"
}

cmd_bump_learn() {
  [[ $# -eq 1 && "$1" =~ ^[0-9]+$ ]] || usage
  local delta="$1" current
  current="$("$DB_SH" meta_get jobs_since_last_learn)"
  [[ "$current" =~ ^[0-9]+$ ]] || current=0
  "$DB_SH" meta_set jobs_since_last_learn "$((current + delta))"
}

main() {
  local sub="${1:-}"
  [[ -n "$sub" ]] || usage
  shift
  case "$sub" in
    attempted_many) cmd_attempted_many "$@" ;;
    skip_job)       cmd_skip_job "$@" ;;
    score_job)      cmd_score_job "$@" ;;
    fail_attempt)   cmd_fail_attempt "$@" ;;
    bump_learn)     cmd_bump_learn "$@" ;;
    -h|--help|help) usage ;;
    *)              usage ;;
  esac
}

main "$@"
