#!/usr/bin/env bash
# Shared SQLite setup for Jessy helpers.

sqlite_require() {
  command -v sqlite3 >/dev/null 2>&1 || {
    echo "${0##*/}: sqlite3 not on PATH (try: brew install sqlite3)" >&2
    exit 3
  }
}

sqlite_check_writable() {
  local db_path="$1" db_dir
  db_dir="$(dirname "$db_path")"
  mkdir -p "$db_dir"
  [[ -w "$db_dir" ]] || {
    echo "${0##*/}: DB directory not writable: $db_dir" >&2
    exit 4
  }
  if [[ -e "$db_path" && ! -w "$db_path" ]]; then
    echo "${0##*/}: DB file not writable: $db_path" >&2
    echo "${0##*/}: fix ownership/permissions or move JESSY_DB" >&2
    exit 4
  fi
  local sidecar
  for sidecar in "$db_path-wal" "$db_path-shm"; do
    if [[ -e "$sidecar" && ! -w "$sidecar" ]]; then
      echo "${0##*/}: DB sidecar not writable: $sidecar" >&2
      echo "${0##*/}: close other Jessy sessions, then fix ownership/permissions" >&2
      exit 4
    fi
  done
}

sqlite_open() {
  sqlite_require
  local db_path="$1" err rc
  shift
  err="$(mktemp)"
  if sqlite3 -cmd '.timeout 15000' -bail -batch "$db_path" "$@" 2>"$err"; then
    rm -f "$err"
    return 0
  fi
  rc=$?
  if grep -qi 'readonly\|read-only\|write-protected' "$err"; then
    echo "${0##*/}: DB write-protected: $db_path" >&2
  elif grep -qi 'locked\|busy' "$err"; then
    echo "${0##*/}: DB locked after 15s timeout: $db_path" >&2
    echo "${0##*/}: close other Jessy/Claude sessions or retry" >&2
  fi
  cat "$err" >&2
  rm -f "$err"
  return "$rc"
}

sqlite_init_db() {
  local db_path="$1" schema="$2"
  sqlite_require
  sqlite_check_writable "$db_path"
  sqlite_open "$db_path" 'PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;' >/dev/null
  sqlite_open "$db_path" < "$schema"
}
