#!/usr/bin/env bash
# Smoke-test the repeat-run boundary contract without Chrome.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB="$ROOT/plugin/scripts/db.sh"
SCAN="$ROOT/plugin/scripts/db_scan.sh"
STAGE="$ROOT/plugin/scripts/db_stage.sh"
TMP_DB="$(mktemp)"
trap 'rm -f "$TMP_DB"' EXIT

export JESSY_DB="$TMP_DB"

old_url="https://www.linkedin.com/jobs/view/old"
newer_url="https://www.linkedin.com/jobs/view/newer"
payload="BOUNDARY_SENTINEL lower card description"

"$DB" init
"$SCAN" score_job linkedin "$old_url" Acme unknown Engineer "$payload" '[]' '[]' 80 good '{"status":"ok"}' >/dev/null

attempts="$("$SCAN" attempted_many "$old_url" "$newer_url")"
grep -q $'https://www.linkedin.com/jobs/view/old\tyes' <<<"$attempts"
grep -q $'https://www.linkedin.com/jobs/view/newer\tno' <<<"$attempts"

# Browser-worker contract: first visible attempted card stops lower cards.
first_status="$(awk 'NR == 1 { print $2 }' <<<"$attempts")"
[[ "$first_status" == "yes" ]]

run_receipt="$("$STAGE" prepare_run repeat-boundary)"
supervisor_transcript="$run_receipt
{\"agent\":\"jessy-browser-worker\",\"status\":\"ok\",\"claimed\":1,\"wrote\":0,\"failed\":0,\"done\":true,\"reason\":\"history_boundary\"}"

! grep -q 'BOUNDARY_SENTINEL' <<<"$supervisor_transcript"
! grep -q 'lower card description' <<<"$supervisor_transcript"
grep -q '"reason":"history_boundary"' <<<"$supervisor_transcript"
