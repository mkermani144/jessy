#!/usr/bin/env bash
# Stress SQLite helpers from parallel shell processes.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB="$ROOT/plugin/scripts/db.sh"
SCAN="$ROOT/plugin/scripts/db_scan.sh"
STAGE="$ROOT/plugin/scripts/db_stage.sh"
TMP_DB="$(mktemp)"
TMP_DIR="$(mktemp -d)"
trap 'rm -f "$TMP_DB" "$TMP_DB-wal" "$TMP_DB-shm"; rm -rf "$TMP_DIR"' EXIT

"$DB" --db "$TMP_DB" init
prepare="$("$STAGE" --db "$TMP_DB" prepare_run concurrency)"
run_id="$(sqlite3 "$TMP_DB" 'SELECT id FROM runs ORDER BY id DESC LIMIT 1;')"

for i in $(seq 1 20); do
  "$STAGE" --db "$TMP_DB" enqueue "$run_id" judge "detail_snapshot:$i" >/dev/null
done

for i in $(seq 1 10); do
  "$STAGE" --db "$TMP_DB" claim_batch "$run_id" judge 2 "worker-$i" \
    > "$TMP_DIR/claim-$i.out" 2> "$TMP_DIR/claim-$i.err" &
done
wait

if grep -R . "$TMP_DIR"/*.err >/dev/null 2>&1; then
  cat "$TMP_DIR"/*.err >&2
  exit 1
fi

claimed="$(sqlite3 "$TMP_DB" "SELECT COUNT(*) FROM stage_items WHERE run_id = $run_id AND stage = 'judge' AND status = 'claimed';")"
pending="$(sqlite3 "$TMP_DB" "SELECT COUNT(*) FROM stage_items WHERE run_id = $run_id AND stage = 'judge' AND status = 'pending';")"
dupes="$(sqlite3 "$TMP_DB" "SELECT COUNT(*) - COUNT(DISTINCT input_ref) FROM stage_items WHERE run_id = $run_id AND stage = 'judge';")"
[[ "$claimed" == "20" ]]
[[ "$pending" == "0" ]]
[[ "$dupes" == "0" ]]

rm -f "$TMP_DIR"/*.out "$TMP_DIR"/*.err
for i in $(seq 1 20); do
  "$STAGE" --db "$TMP_DB" event "$run_id" stress info "event-$i" \
    > "$TMP_DIR/event-$i.out" 2> "$TMP_DIR/event-$i.err" &
done
wait

if grep -R . "$TMP_DIR"/*.err >/dev/null 2>&1; then
  cat "$TMP_DIR"/*.err >&2
  exit 1
fi

events="$(sqlite3 "$TMP_DB" "SELECT COUNT(*) FROM stage_events WHERE run_id = $run_id AND stage = 'stress';")"
[[ "$events" == "20" ]]

rm -f "$TMP_DIR"/*.out "$TMP_DIR"/*.err
for i in $(seq 1 20); do
  "$SCAN" --db "$TMP_DB" skip_job linkedin "https://example.test/jobs/$i" "Co$i" "Skip $i" "desc" 0 "skip" \
    > "$TMP_DIR/scan-$i.out" 2> "$TMP_DIR/scan-$i.err" &
done
wait

if grep -R . "$TMP_DIR"/*.err >/dev/null 2>&1; then
  cat "$TMP_DIR"/*.err >&2
  exit 1
fi

attempts="$(sqlite3 "$TMP_DB" "SELECT COUNT(*) FROM job_attempts WHERE url LIKE 'https://example.test/jobs/%';")"
jobs="$(sqlite3 "$TMP_DB" "SELECT COUNT(*) FROM jobs WHERE url LIKE 'https://example.test/jobs/%';")"
[[ "$attempts" == "20" ]]
[[ "$jobs" == "20" ]]

mode="$(sqlite3 "$TMP_DB" 'PRAGMA journal_mode;')"
[[ "$mode" == "wal" ]]
