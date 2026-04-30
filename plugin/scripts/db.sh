#!/usr/bin/env bash
# jessy db.sh — full surface (rounds 1-4).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA="$SCRIPT_DIR/schema.sql"
source "$SCRIPT_DIR/sqlite_common.sh"

: "${JESSY_DB:=$HOME/.jessy/jessy.db}"

usage() {
  cat >&2 <<'EOF'
usage: db.sh <subcommand> [args...]

subcommands:
  init                              create DB and apply schema (idempotent)
  meta_get <key>                    print meta value (empty if absent; exit 0)
  meta_set <key> <val>              upsert meta value
  attempted <url>                   print "yes" or "no" if URL was attempted
  attempt_start <url> [platform]    persist scan attempt start; prints inserted/skipped
  attempt_finish <url> <status> [extraction_json] [score] [rationale]
                                    persist extraction/scoring outcome
  seen <url>                        alias for attempted (backcompat)
  upsert_company <name> [size] [summary]
                                    insert or update company; print id
  insert_job <url> <company_id> <title> <desc> <req_hard> <req_nice> \
             <platform> <score> <rationale>
                                    insert job row (OR IGNORE on url PK);
                                    ts = now; user_action NULL;
                                    prints "inserted" or "skipped" (exit 0)
  count                             print total jobs row count
  query_report [unseen|all]         emit JSONL, unseen = user_action IS NULL
                                    (default unseen), sorted by score DESC
  consume_report [picked_url ...]   read query_report JSONL snapshot on stdin;
                                    in one transaction, set picked URLs opened
                                    and all other snapshot URLs dismissed;
                                    prints "opened N; dismissed M; unseen 0."
  recent_actions [limit]            JSONL of jobs WHERE user_action IS NOT NULL
                                    ORDER BY ts DESC LIMIT N (default 50)
  cleanup <max_age_days> <max_rows> prune jobs older than N days with action set;
                                    if still over max_rows, drop oldest acted-on
                                    rows. Never deletes user_action IS NULL.
  config_cadence [path]             read learning.cadence array from
                                    ~/.jessy/config.yaml (or <path>) as
                                    newline-separated ints; 0 rows = empty

env:
  JESSY_DB                          DB path (default: ~/.jessy/jessy.db)
EOF
  exit 2
}

cmd_init() {
  sqlite_init_db "$JESSY_DB" "$SCHEMA"
}

db() {
  sqlite_open "$JESSY_DB" "$@"
}

ensure_db_ready() {
  [[ -e "$JESSY_DB" ]] || { cmd_init; return; }
  local has_meta
  has_meta="$(db "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta' LIMIT 1;")"
  [[ "$has_meta" == "1" ]] || cmd_init
}

cmd_meta_get() {
  # Print the value (or empty if missing). Always exit 0 — makes callers
  # set -e safe without needing `|| true` guards.
  local key="${1:-}"
  [[ -n "$key" ]] || { echo "db.sh: meta_get requires <key>" >&2; exit 2; }
  local k
  k=$(printf '%s' "$key" | sed "s/'/''/g")
  db \
    "SELECT value FROM meta WHERE key = '$k';"
}

cmd_meta_set() {
  local key="${1:-}" val="${2:-}"
  [[ -n "$key" ]] || { echo "db.sh: meta_set requires <key> <value>" >&2; exit 2; }
  local k v
  k=$(printf '%s' "$key" | sed "s/'/''/g")
  v=$(printf '%s' "$val" | sed "s/'/''/g")
  db \
    "INSERT INTO meta(key, value) VALUES('$k', '$v')
     ON CONFLICT(key) DO UPDATE SET value=excluded.value;"
}

sql_quote() {
  # bash quote → SQL single-quoted literal. Use as: $(sql_quote "$x")
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/''/g")"
}

cmd_seen() {
  local url="${1:-}"
  [[ -n "$url" ]] || { echo "db.sh: seen requires <url>" >&2; exit 2; }
  local out
  out=$(db \
    "SELECT 1
     WHERE EXISTS (SELECT 1 FROM job_attempts WHERE url = $(sql_quote "$url"))
        OR EXISTS (SELECT 1 FROM jobs WHERE url = $(sql_quote "$url"))
     LIMIT 1;")
  if [[ "$out" == "1" ]]; then
    echo yes
  else
    echo no
  fi
}

cmd_attempted() {
  # Boundary check for normal scans. Any attempt row counts, including
  # started/failed rows; legacy jobs rows also count.
  cmd_seen "$@"
}

cmd_attempt_start() {
  local url="${1:-}" platform="${2:-linkedin}"
  [[ -n "$url" ]] || { echo "db.sh: attempt_start requires <url> [platform]" >&2; exit 2; }
  local before after
  before=$(db "SELECT COUNT(*) FROM job_attempts;")
  db <<SQL
INSERT OR IGNORE INTO job_attempts(url, platform, status, started_ts)
VALUES(
  $(sql_quote "$url"),
  $(sql_quote "$platform"),
  'started',
  CAST(strftime('%s','now') AS INTEGER)
);
SQL
  after=$(db "SELECT COUNT(*) FROM job_attempts;")
  if [[ "$after" -gt "$before" ]]; then
    echo inserted
  else
    echo skipped
  fi
}

cmd_attempt_finish() {
  local url="${1:-}" status="${2:-}" extraction_json="${3:-}" score="${4:-}" rationale="${5:-}"
  [[ -n "$url" && -n "$status" ]] || {
    echo "db.sh: attempt_finish requires <url> <status> [extraction_json] [score] [rationale]" >&2
    exit 2
  }
  case "$status" in
    ok|partial|failed|scored|accepted|rejected|deferred) ;;
    *) echo "db.sh: attempt status invalid" >&2; exit 2 ;;
  esac
  if [[ -n "$score" && ! "$score" =~ ^-?[0-9]+$ ]]; then
    echo "db.sh: score must be int" >&2
    exit 2
  fi
  local score_sql="NULL"
  [[ -n "$score" ]] && score_sql="$score"
  db <<SQL
INSERT INTO job_attempts(
  url, platform, status, started_ts, finished_ts, error,
  extraction_json, score, rationale
)
VALUES(
  $(sql_quote "$url"),
  'linkedin',
  $(sql_quote "$status"),
  CAST(strftime('%s','now') AS INTEGER),
  CAST(strftime('%s','now') AS INTEGER),
  CASE WHEN $(sql_quote "$status") = 'failed' THEN $(sql_quote "$rationale") ELSE NULL END,
  $(sql_quote "$extraction_json"),
  $score_sql,
  $(sql_quote "$rationale")
)
ON CONFLICT(url) DO UPDATE SET
  status = excluded.status,
  finished_ts = excluded.finished_ts,
  error = excluded.error,
  extraction_json = excluded.extraction_json,
  score = excluded.score,
  rationale = excluded.rationale;
SQL
}

cmd_upsert_company() {
  local name="${1:-}" size="${2:-}" summary="${3:-}"
  [[ -n "$name" ]] || { echo "db.sh: upsert_company requires <name>" >&2; exit 2; }
  db <<SQL
INSERT INTO companies(name, size, summary)
VALUES($(sql_quote "$name"), $(sql_quote "$size"), $(sql_quote "$summary"))
ON CONFLICT(name) DO UPDATE SET
  size    = COALESCE(NULLIF(excluded.size, ''), companies.size),
  summary = COALESCE(NULLIF(excluded.summary, ''), companies.summary);
SELECT id FROM companies WHERE name = $(sql_quote "$name");
SQL
}

cmd_insert_job() {
  if [[ $# -lt 9 ]]; then
    echo "db.sh: insert_job needs 9 args (url company_id title desc req_hard req_nice platform score rationale)" >&2
    exit 2
  fi
  local url="$1" company_id="$2" title="$3" desc="$4"
  local req_hard="$5" req_nice="$6" platform="$7" score="$8" rationale="$9"
  # Validate numeric fields (defensive — Claude builds these)
  [[ "$company_id" =~ ^[0-9]+$ ]] || { echo "db.sh: company_id must be int" >&2; exit 2; }
  [[ "$score" =~ ^-?[0-9]+$ ]] || { echo "db.sh: score must be int" >&2; exit 2; }
  local before after
  before=$(db "SELECT COUNT(*) FROM jobs;")
  db <<SQL
INSERT OR IGNORE INTO jobs(url, company_id, title, desc, req_hard, req_nice, platform, score, rationale, ts)
VALUES(
  $(sql_quote "$url"),
  $company_id,
  $(sql_quote "$title"),
  $(sql_quote "$desc"),
  $(sql_quote "$req_hard"),
  $(sql_quote "$req_nice"),
  $(sql_quote "$platform"),
  $score,
  $(sql_quote "$rationale"),
  CAST(strftime('%s','now') AS INTEGER)
);
INSERT INTO job_attempts(url, platform, status, started_ts, finished_ts, score, rationale)
VALUES(
  $(sql_quote "$url"),
  $(sql_quote "$platform"),
  'scored',
  CAST(strftime('%s','now') AS INTEGER),
  CAST(strftime('%s','now') AS INTEGER),
  $score,
  $(sql_quote "$rationale")
)
ON CONFLICT(url) DO UPDATE SET
  status = 'scored',
  finished_ts = excluded.finished_ts,
  score = excluded.score,
  rationale = excluded.rationale;
SQL
  after=$(db "SELECT COUNT(*) FROM jobs;")
  if [[ "$after" -gt "$before" ]]; then
    echo inserted
  else
    echo skipped
  fi
}

cmd_count() {
  db "SELECT COUNT(*) FROM jobs;"
}

cmd_query_report() {
  local scope="${1:-unseen}"
  local where
  case "$scope" in
    unseen) where="WHERE j.user_action IS NULL" ;;
    all)    where="" ;;
    *) echo "db.sh: query_report scope must be unseen|all" >&2; exit 2 ;;
  esac
  db <<SQL
.mode list
.separator "\n"
SELECT json_object(
  'url',             j.url,
  'title',           j.title,
  'desc',            COALESCE(j.desc, ''),
  'req_hard',        COALESCE(j.req_hard, '[]'),
  'req_nice',        COALESCE(j.req_nice, '[]'),
  'platform',        j.platform,
  'score',           j.score,
  'rationale',       COALESCE(j.rationale, ''),
  'user_action',     j.user_action,
  'ts',              j.ts,
  'company_name',    COALESCE(c.name, ''),
  'company_size',    COALESCE(c.size, ''),
  'company_summary', COALESCE(c.summary, ''),
  'extract_status',  COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.status') END, ''),
  'extract_lang',    COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.lang') END, ''),
  'location',        COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.location') END, ''),
  'seniority',       COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.seniority') END, ''),
  'employment',      COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.employment') END, ''),
  'salary',          COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.salary') END, ''),
  'visa',            COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.visa') END, ''),
  'extract_summary', COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.summary') END, '[]'),
  'evidence',        COALESCE(CASE WHEN json_valid(a.extraction_json) THEN json_extract(a.extraction_json, '$.evidence') END, '[]')
)
FROM jobs j
LEFT JOIN companies c ON c.id = j.company_id
LEFT JOIN job_attempts a ON a.url = j.url
$where
ORDER BY j.score DESC, j.ts DESC;
SQL
}

cmd_consume_report() {
  # Consume exactly the report snapshot passed on stdin. Picked URLs are args;
  # every other URL from the snapshot is dismissed. This keeps report handling
  # deterministic even if later scans add more unseen jobs.
  local sql line url
  sql=$(
    cat <<'SQL'
BEGIN IMMEDIATE;
CREATE TEMP TABLE report_lines(line TEXT NOT NULL);
CREATE TEMP TABLE report_urls(url TEXT PRIMARY KEY NOT NULL);
CREATE TEMP TABLE picked_urls(url TEXT PRIMARY KEY NOT NULL);
SQL
  )

  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    sql+=$'\n'"INSERT INTO report_lines(line) VALUES($(sql_quote "$line"));"
  done

  for url in "$@"; do
    [[ -n "$url" ]] || continue
    sql+=$'\n'"INSERT OR IGNORE INTO picked_urls(url) VALUES($(sql_quote "$url"));"
  done

  sql+=$'\n'
  sql+=$(cat <<'SQL'
INSERT OR IGNORE INTO report_urls(url)
SELECT json_extract(line, '$.url')
FROM report_lines
WHERE NULLIF(json_extract(line, '$.url'), '') IS NOT NULL;

UPDATE jobs
SET user_action = CASE
  WHEN url IN (SELECT url FROM picked_urls) THEN 'opened'
  ELSE 'dismissed'
END
WHERE url IN (SELECT url FROM report_urls);

COMMIT;

SELECT printf(
  'opened %d; dismissed %d; unseen 0.',
  (SELECT COUNT(*) FROM report_urls WHERE url IN (SELECT url FROM picked_urls)),
  (SELECT COUNT(*) FROM report_urls WHERE url NOT IN (SELECT url FROM picked_urls))
);
SQL
)

  db <<<"$sql"
}

cmd_recent_actions() {
  local limit="${1:-50}"
  [[ "$limit" =~ ^[0-9]+$ ]] || { echo "db.sh: limit must be int" >&2; exit 2; }
  db <<SQL
.mode list
.separator "\n"
SELECT json_object(
  'url',             j.url,
  'title',           j.title,
  'desc',            COALESCE(j.desc, ''),
  'req_hard',        COALESCE(j.req_hard, '[]'),
  'req_nice',        COALESCE(j.req_nice, '[]'),
  'platform',        j.platform,
  'score',           j.score,
  'rationale',       COALESCE(j.rationale, ''),
  'user_action',     j.user_action,
  'ts',              j.ts,
  'company_name',    COALESCE(c.name, ''),
  'company_size',    COALESCE(c.size, ''),
  'company_summary', COALESCE(c.summary, '')
)
FROM jobs j
LEFT JOIN companies c ON c.id = j.company_id
WHERE j.user_action IS NOT NULL
ORDER BY j.ts DESC
LIMIT $limit;
SQL
}

cmd_cleanup() {
  local max_age="${1:-}" max_rows="${2:-}"
  [[ "$max_age"  =~ ^[0-9]+$ ]] || { echo "db.sh: max_age_days must be int" >&2; exit 2; }
  [[ "$max_rows" =~ ^[0-9]+$ ]] || { echo "db.sh: max_rows must be int" >&2; exit 2; }
  local cutoff before after
  cutoff=$(( $(date +%s) - max_age * 86400 ))
  before=$(db "SELECT COUNT(*) FROM jobs;")

  # 1. Age-based prune (acted-on only)
  db \
    "DELETE FROM jobs WHERE user_action IS NOT NULL AND ts < $cutoff;"

  # 2. Row-cap prune — compute extra in shell to avoid SQLite LIMIT
  #    gotchas (negative LIMIT = unlimited; MAX() aggregate ambiguity).
  local now_rows extra
  now_rows=$(db "SELECT COUNT(*) FROM jobs;")
  extra=$(( now_rows - max_rows ))
  if [[ $extra -gt 0 ]]; then
    db <<SQL
DELETE FROM jobs WHERE url IN (
  SELECT url FROM jobs
  WHERE user_action IS NOT NULL
  ORDER BY ts ASC
  LIMIT $extra
);
SQL
  fi

  # 3. Orphan companies (keep only ones referenced by remaining jobs)
  db \
    "DELETE FROM companies
     WHERE id NOT IN (SELECT DISTINCT company_id FROM jobs WHERE company_id IS NOT NULL);"

  after=$(db "SELECT COUNT(*) FROM jobs;")
  printf 'pruned %d; now %d rows\n' "$((before - after))" "$after"
}

cmd_config_cadence() {
  # Parse `cadence: [N, M, ...]` from the `learning:` block of config.yaml.
  # Accepts inline-array form only (the template format). Empty / missing
  # → no output (caller treats as "cadence disabled").
  local path="${1:-$HOME/.jessy/config.yaml}"
  [[ -f "$path" ]] || { return 0; }
  awk '
    /^learning:/      { in_learning=1; next }
    in_learning && /^[^[:space:]]/ { in_learning=0 }
    in_learning && /^[[:space:]]+cadence:[[:space:]]*\[/ {
      s = $0
      sub(/^[^\[]*\[/, "", s)
      sub(/\].*$/, "", s)
      n = split(s, a, ",")
      for (i = 1; i <= n; i++) {
        gsub(/[[:space:]]/, "", a[i])
        if (a[i] != "") print a[i]
      }
      exit
    }
  ' "$path"
}

main() {
  local sub="${1:-}"
  [[ -n "$sub" ]] || usage
  shift
  # Keep old installs migrated when new subcommands hit existing DBs.
  case "$sub" in
    init|config_cadence|-h|--help|help) ;;
    *) ensure_db_ready ;;
  esac
  case "$sub" in
    init)            cmd_init "$@" ;;
    meta_get)        cmd_meta_get "$@" ;;
    meta_set)        cmd_meta_set "$@" ;;
    attempted)       cmd_attempted "$@" ;;
    attempt_start)   cmd_attempt_start "$@" ;;
    attempt_finish)  cmd_attempt_finish "$@" ;;
    seen)            cmd_seen "$@" ;;
    upsert_company)  cmd_upsert_company "$@" ;;
    insert_job)      cmd_insert_job "$@" ;;
    count)           cmd_count "$@" ;;
    query_report)    cmd_query_report "$@" ;;
    consume_report)  cmd_consume_report "$@" ;;
    recent_actions)  cmd_recent_actions "$@" ;;
    cleanup)         cmd_cleanup "$@" ;;
    config_cadence)  cmd_config_cadence "$@" ;;
    -h|--help|help)  usage ;;
    *)               echo "db.sh: unknown subcommand: $sub" >&2; usage ;;
  esac
}

main "$@"
