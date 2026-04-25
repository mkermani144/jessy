#!/usr/bin/env bash
# jessy db.sh — full surface (rounds 1-4).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA="$SCRIPT_DIR/schema.sql"

: "${JESSY_DB:=$HOME/.jessy/jessy.db}"

usage() {
  cat >&2 <<'EOF'
usage: db.sh <subcommand> [args...]

subcommands:
  init                              create DB and apply schema (idempotent)
  meta_get <key>                    print meta value (empty if absent; exit 0)
  meta_set <key> <val>              upsert meta value
  seen <url>                        print "yes" or "no" (exit 0 in both cases)
  company_exists <name>             print "yes" or "no" (exit 0 in both cases)
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
  mark_action <url> <opened|dismissed>
                                    set jobs.user_action for url
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

require_sqlite() {
  command -v sqlite3 >/dev/null 2>&1 || {
    echo "db.sh: sqlite3 not on PATH (try: brew install sqlite3)" >&2
    exit 3
  }
}

cmd_init() {
  require_sqlite
  mkdir -p "$(dirname "$JESSY_DB")"
  sqlite3 -bail -batch "$JESSY_DB" < "$SCHEMA"
}

cmd_meta_get() {
  # Print the value (or empty if missing). Always exit 0 — makes callers
  # set -e safe without needing `|| true` guards.
  require_sqlite
  local key="${1:-}"
  [[ -n "$key" ]] || { echo "db.sh: meta_get requires <key>" >&2; exit 2; }
  local k
  k=$(printf '%s' "$key" | sed "s/'/''/g")
  sqlite3 -bail -batch "$JESSY_DB" \
    "SELECT value FROM meta WHERE key = '$k';"
}

cmd_meta_set() {
  require_sqlite
  local key="${1:-}" val="${2:-}"
  [[ -n "$key" ]] || { echo "db.sh: meta_set requires <key> <value>" >&2; exit 2; }
  local k v
  k=$(printf '%s' "$key" | sed "s/'/''/g")
  v=$(printf '%s' "$val" | sed "s/'/''/g")
  sqlite3 -bail -batch "$JESSY_DB" \
    "INSERT INTO meta(key, value) VALUES('$k', '$v')
     ON CONFLICT(key) DO UPDATE SET value=excluded.value;"
}

sql_quote() {
  # bash quote → SQL single-quoted literal. Use as: $(sql_quote "$x")
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/''/g")"
}

cmd_seen() {
  require_sqlite
  local url="${1:-}"
  [[ -n "$url" ]] || { echo "db.sh: seen requires <url>" >&2; exit 2; }
  local out
  out=$(sqlite3 -bail -batch "$JESSY_DB" \
    "SELECT 1 FROM jobs WHERE url = $(sql_quote "$url") LIMIT 1;")
  if [[ "$out" == "1" ]]; then
    echo yes
  else
    echo no
  fi
}

# Print "yes"/"no" if a company row with this name exists.
cmd_company_exists() {
  require_sqlite
  local name="${1:-}"
  [[ -n "$name" ]] || { echo "db.sh: company_exists requires <name>" >&2; exit 2; }
  local out
  out=$(sqlite3 -bail -batch "$JESSY_DB" \
    "SELECT 1 FROM companies WHERE name = $(sql_quote "$name") LIMIT 1;")
  if [[ "$out" == "1" ]]; then
    echo yes
  else
    echo no
  fi
}

cmd_upsert_company() {
  require_sqlite
  local name="${1:-}" size="${2:-}" summary="${3:-}"
  [[ -n "$name" ]] || { echo "db.sh: upsert_company requires <name>" >&2; exit 2; }
  sqlite3 -bail -batch "$JESSY_DB" <<SQL
INSERT INTO companies(name, size, summary)
VALUES($(sql_quote "$name"), $(sql_quote "$size"), $(sql_quote "$summary"))
ON CONFLICT(name) DO UPDATE SET
  size    = COALESCE(NULLIF(excluded.size, ''), companies.size),
  summary = COALESCE(NULLIF(excluded.summary, ''), companies.summary);
SELECT id FROM companies WHERE name = $(sql_quote "$name");
SQL
}

cmd_insert_job() {
  require_sqlite
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
  before=$(sqlite3 -bail -batch "$JESSY_DB" "SELECT COUNT(*) FROM jobs;")
  sqlite3 -bail -batch "$JESSY_DB" <<SQL
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
SQL
  after=$(sqlite3 -bail -batch "$JESSY_DB" "SELECT COUNT(*) FROM jobs;")
  if [[ "$after" -gt "$before" ]]; then
    echo inserted
  else
    echo skipped
  fi
}

cmd_count() {
  require_sqlite
  sqlite3 -bail -batch "$JESSY_DB" "SELECT COUNT(*) FROM jobs;"
}

cmd_query_report() {
  require_sqlite
  local scope="${1:-unseen}"
  local where
  case "$scope" in
    unseen) where="WHERE j.user_action IS NULL" ;;
    all)    where="" ;;
    *) echo "db.sh: query_report scope must be unseen|all" >&2; exit 2 ;;
  esac
  sqlite3 -bail -batch "$JESSY_DB" <<SQL
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
$where
ORDER BY j.score DESC, j.ts DESC;
SQL
}

cmd_recent_actions() {
  require_sqlite
  local limit="${1:-50}"
  [[ "$limit" =~ ^[0-9]+$ ]] || { echo "db.sh: limit must be int" >&2; exit 2; }
  sqlite3 -bail -batch "$JESSY_DB" <<SQL
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
  require_sqlite
  local max_age="${1:-}" max_rows="${2:-}"
  [[ "$max_age"  =~ ^[0-9]+$ ]] || { echo "db.sh: max_age_days must be int" >&2; exit 2; }
  [[ "$max_rows" =~ ^[0-9]+$ ]] || { echo "db.sh: max_rows must be int" >&2; exit 2; }
  local cutoff before after
  cutoff=$(( $(date +%s) - max_age * 86400 ))
  before=$(sqlite3 -bail -batch "$JESSY_DB" "SELECT COUNT(*) FROM jobs;")

  # 1. Age-based prune (acted-on only)
  sqlite3 -bail -batch "$JESSY_DB" \
    "DELETE FROM jobs WHERE user_action IS NOT NULL AND ts < $cutoff;"

  # 2. Row-cap prune — compute extra in shell to avoid SQLite LIMIT
  #    gotchas (negative LIMIT = unlimited; MAX() aggregate ambiguity).
  local now_rows extra
  now_rows=$(sqlite3 -bail -batch "$JESSY_DB" "SELECT COUNT(*) FROM jobs;")
  extra=$(( now_rows - max_rows ))
  if [[ $extra -gt 0 ]]; then
    sqlite3 -bail -batch "$JESSY_DB" <<SQL
DELETE FROM jobs WHERE url IN (
  SELECT url FROM jobs
  WHERE user_action IS NOT NULL
  ORDER BY ts ASC
  LIMIT $extra
);
SQL
  fi

  # 3. Orphan companies (keep only ones referenced by remaining jobs)
  sqlite3 -bail -batch "$JESSY_DB" \
    "DELETE FROM companies
     WHERE id NOT IN (SELECT DISTINCT company_id FROM jobs WHERE company_id IS NOT NULL);"

  after=$(sqlite3 -bail -batch "$JESSY_DB" "SELECT COUNT(*) FROM jobs;")
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

cmd_mark_action() {
  require_sqlite
  local url="${1:-}" action="${2:-}"
  [[ -n "$url" && -n "$action" ]] || {
    echo "db.sh: mark_action requires <url> <opened|dismissed>" >&2
    exit 2
  }
  case "$action" in
    opened|dismissed) ;;
    *) echo "db.sh: action must be opened or dismissed" >&2; exit 2 ;;
  esac
  sqlite3 -bail -batch "$JESSY_DB" \
    "UPDATE jobs SET user_action = $(sql_quote "$action")
     WHERE url = $(sql_quote "$url");"
}

stub() {
  echo "db.sh: subcommand \"$1\" not implemented yet" >&2
  exit 2
}

main() {
  local sub="${1:-}"
  [[ -n "$sub" ]] || usage
  shift
  case "$sub" in
    init)            cmd_init "$@" ;;
    meta_get)        cmd_meta_get "$@" ;;
    meta_set)        cmd_meta_set "$@" ;;
    seen)            cmd_seen "$@" ;;
    company_exists)  cmd_company_exists "$@" ;;
    upsert_company)  cmd_upsert_company "$@" ;;
    insert_job)      cmd_insert_job "$@" ;;
    count)           cmd_count "$@" ;;
    query_report)    cmd_query_report "$@" ;;
    mark_action)     cmd_mark_action "$@" ;;
    recent_actions)  cmd_recent_actions "$@" ;;
    cleanup)         cmd_cleanup "$@" ;;
    config_cadence)  cmd_config_cadence "$@" ;;
    -h|--help|help)  usage ;;
    *)               echo "db.sh: unknown subcommand: $sub" >&2; usage ;;
  esac
}

main "$@"
