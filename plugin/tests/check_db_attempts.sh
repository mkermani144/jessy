#!/usr/bin/env bash
# Verify scan attempt boundary state stays independent from report actions.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB="$ROOT/plugin/scripts/db.sh"
SCAN_DB="$ROOT/plugin/scripts/db_scan.sh"
TMP_DB="$(mktemp)"
trap 'rm -f "$TMP_DB"' EXIT

export JESSY_DB="$TMP_DB"

"$DB" init
[[ "$("$DB" preflight_writable)" == "ok" ]]

[[ "$("$DB" attempted https://www.linkedin.com/jobs/view/1)" == "no" ]]
[[ "$("$DB" attempt_start https://www.linkedin.com/jobs/view/1 linkedin)" == "inserted" ]]
[[ "$("$DB" attempted https://www.linkedin.com/jobs/view/1)" == "yes" ]]
[[ "$("$DB" seen https://www.linkedin.com/jobs/view/1)" == "yes" ]]
[[ "$("$DB" attempt_start https://www.linkedin.com/jobs/view/1 linkedin)" == "skipped" ]]

"$DB" attempt_finish https://www.linkedin.com/jobs/view/1 failed '{"status":"failed"}' '' detail_not_loaded

cid="$("$DB" upsert_company Acme "" "")"
[[ "$("$DB" insert_job https://www.linkedin.com/jobs/view/2 "$cid" Engineer "" '[]' '[]' linkedin 80 good)" == "inserted" ]]
[[ "$("$DB" attempted https://www.linkedin.com/jobs/view/2)" == "yes" ]]

rows="$(sqlite3 "$TMP_DB" 'SELECT COUNT(*) FROM job_attempts;')"
[[ "$rows" == "2" ]]

many="$("$SCAN_DB" attempted_many \
  https://www.linkedin.com/jobs/view/1 \
  https://www.linkedin.com/jobs/view/404)"
grep -q $'https://www.linkedin.com/jobs/view/1\tyes' <<<"$many"
grep -q $'https://www.linkedin.com/jobs/view/404\tno' <<<"$many"

[[ "$("$SCAN_DB" skip_job https://www.linkedin.com/jobs/view/3 Acme Intern "" 0 "skip title: intern")" == "inserted" ]]
[[ "$("$DB" attempted https://www.linkedin.com/jobs/view/3)" == "yes" ]]

extract='{"status":"ok","url":"https://www.linkedin.com/jobs/view/4","lang":"en","location":"remote US","seniority":"senior","employment":"full_time","salary":"unknown","visa":"unknown","summary":["Build APIs"],"evidence":["Remote - United States"]}'
[[ "$("$SCAN_DB" score_job https://www.linkedin.com/jobs/view/4 Acme unknown Engineer desc '["rust"]' '[]' 75 good "$extract")" == "inserted" ]]
[[ "$("$DB" attempted https://www.linkedin.com/jobs/view/4)" == "yes" ]]
report="$("$DB" query_report)"
grep -q '"extract_status":"ok"' <<<"$report"
grep -q '"location":"remote US"' <<<"$report"
grep -q '"seniority":"senior"' <<<"$report"
grep -q '"employment":"full_time"' <<<"$report"
grep -q '"salary":"unknown"' <<<"$report"
grep -q '"visa":"unknown"' <<<"$report"
grep -q '"extract_summary":\["Build APIs"\]' <<<"$report"
grep -q '"evidence":\["Remote - United States"\]' <<<"$report"

"$SCAN_DB" fail_attempt https://www.linkedin.com/jobs/view/5 detail_not_loaded '{"status":"failed"}'
[[ "$("$DB" attempted https://www.linkedin.com/jobs/view/5)" == "yes" ]]

wf_extract='{"status":"ok","url":"https://wellfound.com/jobs/123-engineer","lang":"en","location":"remote US","seniority":"senior","employment":"full_time","salary":"$150k","visa":"unknown","summary":["Build product"],"evidence":["Remote only"]}'
[[ "$("$SCAN_DB" score_job wellfound https://wellfound.com/jobs/123-engineer Acme unknown Engineer desc '["rust"]' '[]' 82 good "$wf_extract")" == "inserted" ]]
wf_platform="$(sqlite3 "$TMP_DB" "SELECT platform FROM jobs WHERE url = 'https://wellfound.com/jobs/123-engineer';")"
[[ "$wf_platform" == "wellfound" ]]
"$SCAN_DB" fail_attempt wellfound https://wellfound.com/jobs/124-engineer detail_not_loaded '{"status":"failed"}'
wf_attempt_platform="$(sqlite3 "$TMP_DB" "SELECT platform FROM job_attempts WHERE url = 'https://wellfound.com/jobs/124-engineer';")"
[[ "$wf_attempt_platform" == "wellfound" ]]

"$DB" meta_set jobs_since_last_learn 2
"$SCAN_DB" bump_learn 3
[[ "$("$DB" meta_get jobs_since_last_learn)" == "5" ]]
