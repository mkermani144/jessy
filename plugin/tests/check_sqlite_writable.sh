#!/usr/bin/env bash
# Verify DB permission failures are diagnosed before SQLite write attempts.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB="$ROOT/plugin/scripts/db.sh"
TMP_DB="$(mktemp)"
TMP_DB2="$(mktemp)"
ERR="$(mktemp)"
trap 'chmod u+w "$TMP_DB" "$TMP_DB-wal" "$TMP_DB-shm" "$TMP_DB2" "$TMP_DB2-wal" "$TMP_DB2-shm" 2>/dev/null || true; rm -f "$TMP_DB" "$TMP_DB-wal" "$TMP_DB-shm" "$TMP_DB2" "$TMP_DB2-wal" "$TMP_DB2-shm" "$ERR"' EXIT

export JESSY_DB="$TMP_DB"

"$DB" init
chmod a-w "$TMP_DB"

if "$DB" init > /dev/null 2>"$ERR"; then
  echo "expected read-only DB init to fail" >&2
  exit 1
fi

grep -q 'DB file not writable' "$ERR"

export JESSY_DB="$TMP_DB2"
"$DB" init
touch "$TMP_DB2-wal"
chmod a-w "$TMP_DB2-wal"
if "$DB" init > /dev/null 2>"$ERR"; then
  echo "expected read-only WAL sidecar init to fail" >&2
  exit 1
fi

grep -q 'DB sidecar not writable' "$ERR"
