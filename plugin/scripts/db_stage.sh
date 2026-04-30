#!/usr/bin/env bash
# Stage-bus helpers. Stdout is always a compact receipt, never payload rows.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_SH="$SCRIPT_DIR/db.sh"
source "$SCRIPT_DIR/sqlite_common.sh"

: "${JESSY_DB:=$HOME/.jessy/jessy.db}"

usage() {
  cat >&2 <<'EOF'
usage: db_stage.sh [--db <path>] <subcommand> [args...]

subcommands:
  prepare_run [config_hash] [browser_input_ref]
  run_create [config_hash]
  run_finish <run_id> <ok|failed|paused> [error]
  event <run_id> <stage> <info|warn|error> <message> [meta_json]
  enqueue <run_id> <stage> [input_ref] [result_meta_json]
  claim <run_id> <stage> [claim_id]
  claim_batch <run_id> <stage> [limit] [claim_id]
  finish <item_id> <done|failed|skipped> [result_meta_json]
  fail <item_id> <error>
  page_snapshot <run_id> <platform> <tab_url> <fingerprint> <snapshot_ref> [snapshot_text]
  job_seed <run_id> <platform> <canonical_url> <title> <company> <location> \
           <snippet> <source_snapshot_id> <rank> <status> [reason]
  detail_snapshot <run_id> <seed_id> <canonical_url> <fetch_status> \
                  <snapshot_ref> [error] [snapshot_text]
  queue_detail <run_id> <seed_id> <canonical_url> <fetch_status> \
               <snapshot_ref> [error] [snapshot_text]
  detail_context <detail_snapshot_id> [text_cap]
  summary <run_id>
EOF
  exit 2
}

parse_global_args() {
  while [[ "${1:-}" == "--db" ]]; do
    [[ -n "${2:-}" ]] || { echo "db_stage.sh: --db requires <path>" >&2; exit 2; }
    JESSY_DB="$2"
    export JESSY_DB
    shift 2
  done
  GLOBAL_ARGS=("$@")
}

init_db() {
  if [[ ! -e "$JESSY_DB" ]]; then
    "$DB_SH" init >/dev/null
    return
  fi
  local has_stage
  has_stage="$(sqlite_open "$JESSY_DB" "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'stage_items' LIMIT 1;")"
  [[ "$has_stage" == "1" ]] || "$DB_SH" init >/dev/null
}

sql_quote() {
  # Bash string to SQL literal.
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/''/g")"
}

require_int() {
  local name="$1" value="$2"
  [[ "$value" =~ ^[0-9]+$ ]] || { echo "db_stage.sh: $name must be int" >&2; exit 2; }
}

require_status() {
  local name="$1" value="$2" allowed="$3"
  [[ " $allowed " == *" $value "* ]] || {
    echo "db_stage.sh: invalid $name" >&2
    exit 2
  }
}

db() {
  sqlite_open "$JESSY_DB" "$@"
}

cmd_run_create() {
  init_db
  local config_hash="${1:-}"
  db <<SQL
INSERT INTO runs(status, started_ts, config_hash)
VALUES('running', CAST(strftime('%s','now') AS INTEGER), $(sql_quote "$config_hash"));
SELECT json_object('status','ok','run_id',last_insert_rowid(),'next','browser');
SQL
}

cmd_prepare_run() {
  init_db
  local config_hash="${1:-}" input_ref="${2:-browser:scan}"
  db <<SQL
BEGIN IMMEDIATE;
INSERT INTO runs(status, started_ts, config_hash)
VALUES('running', CAST(strftime('%s','now') AS INTEGER), $(sql_quote "$config_hash"));
CREATE TEMP TABLE new_run(id INTEGER PRIMARY KEY);
INSERT INTO new_run(id) VALUES(last_insert_rowid());
INSERT INTO stage_items(
  run_id, stage, status, input_ref, created_ts, updated_ts
)
SELECT id, 'browser', 'pending', $(sql_quote "$input_ref"),
       CAST(strftime('%s','now') AS INTEGER),
       CAST(strftime('%s','now') AS INTEGER)
FROM new_run;
INSERT INTO stage_events(run_id, stage, level, message, ts)
SELECT id, 'ops', 'info', 'run prepared', CAST(strftime('%s','now') AS INTEGER)
FROM new_run;
SELECT json_object(
  'status','ok',
  'run_id',(SELECT id FROM new_run),
  'next','browser',
  'browser_items',1
);
COMMIT;
SQL
}

cmd_run_finish() {
  init_db
  local run_id="${1:-}" status="${2:-}" error="${3:-}"
  require_int run_id "$run_id"
  require_status status "$status" "ok failed paused"
  db <<SQL
UPDATE runs
SET status = $(sql_quote "$status"),
    finished_ts = CAST(strftime('%s','now') AS INTEGER),
    error = NULLIF($(sql_quote "$error"), '')
WHERE id = $run_id;
SELECT json_object('status','ok','run_id',$run_id,'run_status',$(sql_quote "$status"));
SQL
}

cmd_event() {
  init_db
  local run_id="${1:-}" stage="${2:-}" level="${3:-}" message="${4:-}" meta="${5:-}"
  require_int run_id "$run_id"
  [[ -n "$stage" && -n "$message" ]] || usage
  require_status level "$level" "info warn error"
  db <<SQL
INSERT INTO stage_events(run_id, stage, level, message, meta, ts)
VALUES($run_id, $(sql_quote "$stage"), $(sql_quote "$level"),
       $(sql_quote "$message"), NULLIF($(sql_quote "$meta"), ''),
       CAST(strftime('%s','now') AS INTEGER));
SELECT json_object('status','ok','run_id',$run_id,'event_id',last_insert_rowid());
SQL
}

cmd_enqueue() {
  init_db
  local run_id="${1:-}" stage="${2:-}" input_ref="${3:-}" result_meta="${4:-}"
  require_int run_id "$run_id"
  [[ -n "$stage" ]] || usage
  db <<SQL
INSERT INTO stage_items(
  run_id, stage, status, input_ref, result_meta, created_ts, updated_ts
)
VALUES(
  $run_id, $(sql_quote "$stage"), 'pending', NULLIF($(sql_quote "$input_ref"), ''),
  NULLIF($(sql_quote "$result_meta"), ''), CAST(strftime('%s','now') AS INTEGER),
  CAST(strftime('%s','now') AS INTEGER)
);
SELECT json_object('status','ok','run_id',$run_id,'item_id',last_insert_rowid(),'stage',$(sql_quote "$stage"));
SQL
}

cmd_claim() {
  init_db
  local run_id="${1:-}" stage="${2:-}" claim_id="${3:-}"
  require_int run_id "$run_id"
  [[ -n "$stage" ]] || usage
  [[ -n "$claim_id" ]] || claim_id="claim-$(date +%s)-$$"
  db <<SQL
BEGIN IMMEDIATE;
CREATE TEMP TABLE claim_target(id INTEGER PRIMARY KEY);
INSERT INTO claim_target(id)
SELECT id FROM stage_items
WHERE run_id = $run_id AND stage = $(sql_quote "$stage") AND status = 'pending'
ORDER BY id
LIMIT 1;
UPDATE stage_items
SET status = 'claimed',
    claim_id = $(sql_quote "$claim_id"),
    attempts = attempts + 1,
    updated_ts = CAST(strftime('%s','now') AS INTEGER)
WHERE id IN (SELECT id FROM claim_target);
SELECT CASE
  WHEN EXISTS (SELECT 1 FROM claim_target) THEN
    json_object(
      'status','ok','run_id',$run_id,'stage',$(sql_quote "$stage"),
      'claimed',1,'done',json('false'),
      'item_id',(SELECT id FROM claim_target),
      'input_ref',(SELECT input_ref FROM stage_items WHERE id = (SELECT id FROM claim_target))
    )
  ELSE
    json_object('status','ok','run_id',$run_id,'stage',$(sql_quote "$stage"),
                'claimed',0,'done',json('true'))
END;
COMMIT;
SQL
}

cmd_claim_batch() {
  init_db
  local run_id="${1:-}" stage="${2:-}" limit="${3:-5}" claim_id="${4:-}"
  require_int run_id "$run_id"
  require_int limit "$limit"
  [[ -n "$stage" ]] || usage
  [[ -n "$claim_id" ]] || claim_id="claim-$(date +%s)-$$"
  db <<SQL
BEGIN IMMEDIATE;
CREATE TEMP TABLE claim_target(id INTEGER PRIMARY KEY);
INSERT INTO claim_target(id)
SELECT id FROM stage_items
WHERE run_id = $run_id AND stage = $(sql_quote "$stage") AND status = 'pending'
ORDER BY id
LIMIT $limit;
UPDATE stage_items
SET status = 'claimed',
    claim_id = $(sql_quote "$claim_id"),
    attempts = attempts + 1,
    updated_ts = CAST(strftime('%s','now') AS INTEGER)
WHERE id IN (SELECT id FROM claim_target);
SELECT json_object(
  'status','ok',
  'run_id',$run_id,
  'stage',$(sql_quote "$stage"),
  'claimed',(SELECT COUNT(*) FROM claim_target),
  'done',json(CASE WHEN (SELECT COUNT(*) FROM claim_target) = 0 THEN 'true' ELSE 'false' END),
  'items',COALESCE((
    SELECT json_group_array(json_object(
      'item_id', s.id,
      'input_ref', s.input_ref
    ))
    FROM stage_items s
    WHERE s.id IN (SELECT id FROM claim_target)
  ), json('[]'))
);
COMMIT;
SQL
}

cmd_finish() {
  init_db
  local item_id="${1:-}" status="${2:-}" result_meta="${3:-}"
  require_int item_id "$item_id"
  require_status status "$status" "done failed skipped"
  db <<SQL
UPDATE stage_items
SET status = $(sql_quote "$status"),
    result_meta = NULLIF($(sql_quote "$result_meta"), ''),
    updated_ts = CAST(strftime('%s','now') AS INTEGER)
WHERE id = $item_id;
SELECT json_object('status','ok','item_id',$item_id,'item_status',$(sql_quote "$status"));
SQL
}

cmd_fail() {
  init_db
  local item_id="${1:-}" error="${2:-}"
  require_int item_id "$item_id"
  [[ -n "$error" ]] || usage
  db <<SQL
UPDATE stage_items
SET status = 'failed',
    result_meta = json_object('error', $(sql_quote "$error")),
    updated_ts = CAST(strftime('%s','now') AS INTEGER)
WHERE id = $item_id;
SELECT json_object('status','ok','item_id',$item_id,'item_status','failed');
SQL
}

cmd_page_snapshot() {
  init_db
  local run_id="${1:-}" platform="${2:-}" tab_url="${3:-}" fingerprint="${4:-}"
  local snapshot_ref="${5:-}" snapshot_text="${6:-}"
  require_int run_id "$run_id"
  [[ -n "$platform" && -n "$tab_url" ]] || usage
  db <<SQL
INSERT INTO page_snapshots(
  run_id, platform, tab_url, fingerprint, snapshot_text, snapshot_ref, captured_ts
)
VALUES($run_id, $(sql_quote "$platform"), $(sql_quote "$tab_url"),
       NULLIF($(sql_quote "$fingerprint"), ''), NULLIF($(sql_quote "$snapshot_text"), ''),
       NULLIF($(sql_quote "$snapshot_ref"), ''), CAST(strftime('%s','now') AS INTEGER));
SELECT json_object('status','ok','run_id',$run_id,'page_snapshot_id',last_insert_rowid());
SQL
}

cmd_job_seed() {
  init_db
  [[ $# -ge 10 ]] || usage
  local run_id="$1" platform="$2" url="$3" title="$4" company="$5" location="$6"
  local snippet="$7" source_id="$8" rank="$9" status="${10}" reason="${11:-}"
  require_int run_id "$run_id"
  require_int source_snapshot_id "$source_id"
  require_int rank "$rank"
  db <<SQL
INSERT INTO job_seeds(
  run_id, platform, canonical_url, title, company, location, snippet,
  source_snapshot_id, rank, status, reason
)
VALUES($run_id, $(sql_quote "$platform"), $(sql_quote "$url"), $(sql_quote "$title"),
       $(sql_quote "$company"), $(sql_quote "$location"), $(sql_quote "$snippet"),
       $source_id, $rank, $(sql_quote "$status"), NULLIF($(sql_quote "$reason"), ''))
ON CONFLICT(run_id, canonical_url) DO UPDATE SET
  title = excluded.title,
  company = excluded.company,
  location = excluded.location,
  snippet = excluded.snippet,
  source_snapshot_id = excluded.source_snapshot_id,
  rank = excluded.rank,
  status = excluded.status,
  reason = excluded.reason;
SELECT json_object('status','ok','run_id',$run_id,'seed_id',
  (SELECT id FROM job_seeds WHERE run_id = $run_id AND canonical_url = $(sql_quote "$url")));
SQL
}

cmd_detail_snapshot() {
  init_db
  local run_id="${1:-}" seed_id="${2:-}" url="${3:-}" fetch_status="${4:-}"
  local snapshot_ref="${5:-}" error="${6:-}" snapshot_text="${7:-}"
  require_int run_id "$run_id"
  require_int seed_id "$seed_id"
  [[ -n "$url" && -n "$fetch_status" ]] || usage
  db <<SQL
INSERT INTO detail_snapshots(
  run_id, seed_id, canonical_url, fetch_status, snapshot_text, snapshot_ref, error, captured_ts
)
VALUES($run_id, $seed_id, $(sql_quote "$url"), $(sql_quote "$fetch_status"),
       NULLIF($(sql_quote "$snapshot_text"), ''), NULLIF($(sql_quote "$snapshot_ref"), ''),
       NULLIF($(sql_quote "$error"), ''), CAST(strftime('%s','now') AS INTEGER));
SELECT json_object('status','ok','run_id',$run_id,'detail_snapshot_id',last_insert_rowid());
SQL
}

cmd_queue_detail() {
  init_db
  local run_id="${1:-}" seed_id="${2:-}" url="${3:-}" fetch_status="${4:-}"
  local snapshot_ref="${5:-}" error="${6:-}" snapshot_text="${7:-}"
  require_int run_id "$run_id"
  require_int seed_id "$seed_id"
  [[ -n "$url" && -n "$fetch_status" ]] || usage
  db <<SQL
BEGIN IMMEDIATE;
INSERT INTO detail_snapshots(
  run_id, seed_id, canonical_url, fetch_status, snapshot_text, snapshot_ref, error, captured_ts
)
VALUES($run_id, $seed_id, $(sql_quote "$url"), $(sql_quote "$fetch_status"),
       NULLIF($(sql_quote "$snapshot_text"), ''), NULLIF($(sql_quote "$snapshot_ref"), ''),
       NULLIF($(sql_quote "$error"), ''), CAST(strftime('%s','now') AS INTEGER));
CREATE TEMP TABLE new_detail(id INTEGER PRIMARY KEY);
INSERT INTO new_detail(id) VALUES(last_insert_rowid());
INSERT INTO stage_items(
  run_id, stage, status, input_ref, created_ts, updated_ts
)
SELECT $run_id, 'judge', 'pending', 'detail_snapshot:' || id,
       CAST(strftime('%s','now') AS INTEGER),
       CAST(strftime('%s','now') AS INTEGER)
FROM new_detail;
SELECT json_object(
  'status','ok',
  'run_id',$run_id,
  'detail_snapshot_id',(SELECT id FROM new_detail),
  'judge_item_id',last_insert_rowid()
);
COMMIT;
SQL
}

cmd_detail_context() {
  init_db
  local detail_id="${1:-}" text_cap="${2:-12000}"
  require_int detail_snapshot_id "$detail_id"
  require_int text_cap "$text_cap"
  db <<SQL
SELECT json_object(
  'status','ok',
  'detail_snapshot_id',d.id,
  'run_id',d.run_id,
  'seed_id',d.seed_id,
  'canonical_url',d.canonical_url,
  'fetch_status',d.fetch_status,
  'snapshot_ref',COALESCE(d.snapshot_ref, ''),
  'snapshot_text',substr(COALESCE(d.snapshot_text, ''), 1, $text_cap),
  'snapshot_truncated',json(CASE WHEN length(COALESCE(d.snapshot_text, '')) > $text_cap THEN 'true' ELSE 'false' END),
  'error',COALESCE(d.error, ''),
  'platform',s.platform,
  'title',COALESCE(s.title, ''),
  'company',COALESCE(s.company, ''),
  'location',COALESCE(s.location, ''),
  'snippet',COALESCE(s.snippet, '')
)
FROM detail_snapshots d
JOIN job_seeds s ON s.id = d.seed_id
WHERE d.id = $detail_id;
SQL
}

cmd_summary() {
  init_db
  local run_id="${1:-}"
  require_int run_id "$run_id"
  db <<SQL
SELECT json_object(
  'status','ok',
  'run_id',$run_id,
  'items',(SELECT COUNT(*) FROM stage_items WHERE run_id = $run_id),
  'pending',(SELECT COUNT(*) FROM stage_items WHERE run_id = $run_id AND status = 'pending'),
  'claimed',(SELECT COUNT(*) FROM stage_items WHERE run_id = $run_id AND status = 'claimed'),
  'done',(SELECT COUNT(*) FROM stage_items WHERE run_id = $run_id AND status = 'done'),
  'failed',(SELECT COUNT(*) FROM stage_items WHERE run_id = $run_id AND status = 'failed'),
  'page_snapshots',(SELECT COUNT(*) FROM page_snapshots WHERE run_id = $run_id),
  'job_seeds',(SELECT COUNT(*) FROM job_seeds WHERE run_id = $run_id),
  'detail_snapshots',(SELECT COUNT(*) FROM detail_snapshots WHERE run_id = $run_id)
);
SQL
}

main() {
  parse_global_args "$@"
  set -- "${GLOBAL_ARGS[@]}"
  local sub="${1:-}"
  [[ -n "$sub" ]] || usage
  shift
  case "$sub" in
    prepare_run)     cmd_prepare_run "$@" ;;
    run_create)      cmd_run_create "$@" ;;
    run_finish)      cmd_run_finish "$@" ;;
    event)           cmd_event "$@" ;;
    enqueue)         cmd_enqueue "$@" ;;
    claim)           cmd_claim "$@" ;;
    claim_batch)     cmd_claim_batch "$@" ;;
    finish)          cmd_finish "$@" ;;
    fail)            cmd_fail "$@" ;;
    page_snapshot)   cmd_page_snapshot "$@" ;;
    job_seed)        cmd_job_seed "$@" ;;
    detail_snapshot) cmd_detail_snapshot "$@" ;;
    queue_detail)    cmd_queue_detail "$@" ;;
    detail_context)  cmd_detail_context "$@" ;;
    summary)         cmd_summary "$@" ;;
    -h|--help|help)  usage ;;
    *)               echo "db_stage.sh: unknown subcommand: $sub" >&2; usage ;;
  esac
}

main "$@"
