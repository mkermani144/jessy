#!/usr/bin/env bash
# Verify batch claims are disjoint before enabling parallel judge workers.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGE="$ROOT/plugin/scripts/db_stage.sh"
TMP_DB="$(mktemp)"
trap 'rm -f "$TMP_DB"' EXIT

export JESSY_DB="$TMP_DB"

"$STAGE" prepare_run claim-test >/dev/null
run_id="$(sqlite3 "$TMP_DB" 'SELECT id FROM runs ORDER BY id DESC LIMIT 1;')"

"$STAGE" enqueue "$run_id" judge detail_snapshot:1 >/dev/null
"$STAGE" enqueue "$run_id" judge detail_snapshot:2 >/dev/null
"$STAGE" enqueue "$run_id" judge detail_snapshot:3 >/dev/null

claim_a="$("$STAGE" claim_batch "$run_id" judge 2 judge-a)"
claim_b="$("$STAGE" claim_batch "$run_id" judge 2 judge-b)"
claim_c="$("$STAGE" claim_batch "$run_id" judge 2 judge-c)"

grep -q '"claimed":2' <<<"$claim_a"
grep -q '"claimed":1' <<<"$claim_b"
grep -q '"claimed":0' <<<"$claim_c"

refs="$(sqlite3 "$TMP_DB" "SELECT claim_id || ':' || input_ref FROM stage_items WHERE run_id = $run_id AND stage = 'judge' ORDER BY input_ref;")"
grep -q 'judge-a:detail_snapshot:1' <<<"$refs"
grep -q 'judge-a:detail_snapshot:2' <<<"$refs"
grep -q 'judge-b:detail_snapshot:3' <<<"$refs"

dupes="$(sqlite3 "$TMP_DB" "SELECT COUNT(*) - COUNT(DISTINCT input_ref) FROM stage_items WHERE run_id = $run_id AND stage = 'judge';")"
[[ "$dupes" == "0" ]]
