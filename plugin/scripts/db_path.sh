#!/usr/bin/env bash
# jessy db_path.sh — print resolved DB path without shell expansion in callers.

set -euo pipefail

printf '%s\n' "${JESSY_DB:-$HOME/.jessy/jessy.db}"
