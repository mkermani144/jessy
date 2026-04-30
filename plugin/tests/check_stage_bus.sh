#!/usr/bin/env bash
# Verify stage bus receipts stay compact while payloads persist in SQLite.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGE="$ROOT/plugin/scripts/db_stage.sh"
TMP_DB="$(mktemp)"
trap 'rm -f "$TMP_DB"' EXIT

export JESSY_DB="$TMP_DB"

prepare_receipt="$("$STAGE" prepare_run hash-1 browser:start)"
grep -q '"status":"ok"' <<<"$prepare_receipt"
grep -q '"next":"browser"' <<<"$prepare_receipt"
prep_run_id="$(sqlite3 "$TMP_DB" 'SELECT id FROM runs WHERE config_hash = "hash-1";')"
[[ "$prep_run_id" == "1" ]]
[[ "$(sqlite3 "$TMP_DB" 'SELECT stage || ":" || status || ":" || input_ref FROM stage_items WHERE run_id = 1;')" == "browser:pending:browser:start" ]]

run_receipt="$("$STAGE" run_create test-hash)"
grep -q '"status":"ok"' <<<"$run_receipt"
run_id="$(sqlite3 "$TMP_DB" 'SELECT id FROM runs WHERE config_hash = "test-hash" LIMIT 1;')"
[[ "$run_id" == "2" ]]

enqueue_receipt="$("$STAGE" enqueue "$run_id" browser tab:1)"
grep -q '"item_id":' <<<"$enqueue_receipt"
item_id="$(sqlite3 "$TMP_DB" "SELECT id FROM stage_items WHERE run_id = $run_id AND input_ref = 'tab:1';")"

claim_receipt="$("$STAGE" claim "$run_id" browser worker-a)"
grep -q '"claimed":1' <<<"$claim_receipt"
grep -q '"input_ref":"tab:1"' <<<"$claim_receipt"
[[ "$(sqlite3 "$TMP_DB" "SELECT attempts FROM stage_items WHERE id = $item_id;")" == "1" ]]

done_receipt="$("$STAGE" finish "$item_id" done '{"wrote":1}')"
grep -q '"item_status":"done"' <<<"$done_receipt"

payload='Visible job detail that must never appear in stdout receipts.'
page_receipt="$("$STAGE" page_snapshot "$run_id" linkedin https://example.test/jobs fp-1 ref-page "$payload")"
grep -q '"page_snapshot_id":1' <<<"$page_receipt"
! grep -q 'Visible job detail' <<<"$page_receipt"
[[ "$(sqlite3 "$TMP_DB" 'SELECT snapshot_text FROM page_snapshots WHERE id = 1;')" == "$payload" ]]

seed_receipt="$("$STAGE" job_seed "$run_id" linkedin https://example.test/jobs/1 Engineer Acme Remote snippet 1 1 pending)"
grep -q '"seed_id":1' <<<"$seed_receipt"

detail_receipt="$("$STAGE" detail_snapshot "$run_id" 1 https://example.test/jobs/1 ok ref-detail "" "$payload")"
grep -q '"detail_snapshot_id":1' <<<"$detail_receipt"
! grep -q 'Visible job detail' <<<"$detail_receipt"

queued_receipt="$("$STAGE" queue_detail "$run_id" 1 https://example.test/jobs/2 ok ref-detail-2 "" "$payload")"
grep -q '"detail_snapshot_id":2' <<<"$queued_receipt"
grep -q '"judge_item_id":' <<<"$queued_receipt"
! grep -q 'Visible job detail' <<<"$queued_receipt"
judge_ref="$(sqlite3 "$TMP_DB" "SELECT input_ref FROM stage_items WHERE run_id = $run_id AND stage = 'judge';")"
[[ "$judge_ref" == "detail_snapshot:2" ]]

empty_claim="$("$STAGE" claim "$run_id" browser worker-b)"
grep -q '"claimed":0' <<<"$empty_claim"
grep -q '"done":true' <<<"$empty_claim"

summary="$("$STAGE" summary "$run_id")"
grep -q '"items":2' <<<"$summary"
grep -q '"page_snapshots":1' <<<"$summary"
grep -q '"job_seeds":1' <<<"$summary"
grep -q '"detail_snapshots":2' <<<"$summary"

finish_run="$("$STAGE" run_finish "$run_id" ok)"
grep -q '"run_status":"ok"' <<<"$finish_run"
