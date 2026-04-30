#!/usr/bin/env bash
# Guard main/supervisor helper outputs from leaking payloads.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB="$ROOT/plugin/scripts/db.sh"
SCAN="$ROOT/plugin/scripts/db_scan.sh"
STAGE="$ROOT/plugin/scripts/db_stage.sh"
REPORT="$ROOT/plugin/scripts/report_session.sh"
TMP_DB="$(mktemp)"
TMP_STATE="$(mktemp)"
trap 'rm -f "$TMP_DB" "$TMP_STATE"' EXIT

export JESSY_DB="$TMP_DB"
export JESSY_REPORT_STATE="$TMP_STATE"
export JESSY_REPORT_NO_TMUX=1

payload="PAYLOAD_SENTINEL full job description should stay persisted only"

assert_no_payload() {
  local text="$1"
  ! grep -q 'PAYLOAD_SENTINEL' <<<"$text"
  ! grep -q 'full job description' <<<"$text"
}

"$DB" init
run_receipt="$("$STAGE" prepare_run compact-test)"
assert_no_payload "$run_receipt"
run_id="$(sqlite3 "$TMP_DB" 'SELECT id FROM runs ORDER BY id DESC LIMIT 1;')"

page_receipt="$("$STAGE" page_snapshot "$run_id" linkedin https://example.test fp ref "$payload")"
assert_no_payload "$page_receipt"

seed_receipt="$("$STAGE" job_seed "$run_id" linkedin https://example.test/1 Title Co Remote snippet 1 1 pending)"
assert_no_payload "$seed_receipt"

detail_receipt="$("$STAGE" queue_detail "$run_id" 1 https://example.test/1 ok ref "" "$payload")"
assert_no_payload "$detail_receipt"

skip_out="$("$SCAN" skip_job linkedin https://example.test/skip Co Skip "$payload" 0 "skip title")"
assert_no_payload "$skip_out"

cid="$("$DB" upsert_company Acme "" "")"
"$DB" insert_job https://example.test/report "$cid" "Report Title" "$payload" '[]' '[]' linkedin 80 good >/dev/null
report_receipt="$("$REPORT" prepare_receipt)"
assert_no_payload "$report_receipt"
! grep -q 'Report Title' <<<"$report_receipt"
! grep -q 'https://example.test/report' <<<"$report_receipt"
