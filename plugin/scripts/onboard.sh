#!/usr/bin/env bash
# jessy onboarding — first-run setup for ~/.jessy/
# Two input modes:
#   TTY / interactive:       prompts from stdin
#   --non-interactive:       read values from --*-file flags (for Claude Code)
# Idempotent: re-run is safe. --force backs up + re-prompts.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DB_SH="$SCRIPT_DIR/db.sh"
CONFIG_TEMPLATE="$PLUGIN_ROOT/config/config.example.yaml"
PREFS_TEMPLATE="$PLUGIN_ROOT/config/preferences.example.md"

JESSY_DIR="$HOME/.jessy"
CONFIG_FILE="$JESSY_DIR/config.yaml"
PREFS_FILE="$JESSY_DIR/preferences.md"
DB_FILE="$JESSY_DIR/jessy.db"

# Hard deps checked up front — before any file writes
command -v jq >/dev/null 2>&1 || {
  printf 'onboard.sh: jq required (brew install jq)\n' >&2
  exit 3
}
command -v sqlite3 >/dev/null 2>&1 || {
  printf 'onboard.sh: sqlite3 required (brew install sqlite3)\n' >&2
  exit 3
}

FORCE=0
NON_INTERACTIVE=0
URLS_FILE_ARG=""
WF_URLS_FILE_ARG=""
DB_FILE_ARG=""
LK_FILE_ARG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)             FORCE=1; shift ;;
    --non-interactive)   NON_INTERACTIVE=1; shift ;;
    --urls-file)         URLS_FILE_ARG="$2"; shift 2 ;;
    --wellfound-urls-file) WF_URLS_FILE_ARG="$2"; shift 2 ;;
    --dealbreakers-file) DB_FILE_ARG="$2";   shift 2 ;;
    --likes-file)        LK_FILE_ARG="$2";   shift 2 ;;
    -h|--help)
      cat <<'EOF'
usage: onboard.sh [--force] [--non-interactive \
                             --urls-file <path> \
                             --wellfound-urls-file <path> \
                             --dealbreakers-file <path> \
                             --likes-file <path>]

Creates ~/.jessy/{config.yaml,preferences.md,jessy.db}.

Interactive mode (default): prompts from stdin. Each prompt reads lines
  until a blank line. Requires a TTY.

Non-interactive mode (--non-interactive): reads values from files.
  Each file holds one entry per line. --urls-file is LinkedIn for backwards
  compatibility. Missing / empty files = empty input.
  Designed for Claude Code's Bash tool which has no TTY.

Re-runs are no-ops unless --force, which backs up existing files first.
EOF
      exit 0
      ;;
    *) echo "onboard.sh: unknown arg: $1" >&2; exit 2 ;;
  esac
done

prompt_lines() {
  # interactive: $1 = prompt label, $2 = optional ERE filter.
  # Reads stdin lines until blank. Prints valid lines to stdout.
  local label="$1" filter="${2:-}"
  printf '\n%s (one per line, blank to end):\n' "$label" >&2
  local line
  while IFS= read -r line; do
    [[ -z "$line" ]] && break
    if [[ -n "$filter" ]] && ! grep -Eq "$filter" <<<"$line"; then
      printf '  invalid (must match %s): %s\n' "$filter" "$line" >&2
      continue
    fi
    printf '%s\n' "$line"
  done
}

copy_if_file() {
  # $1 = src path (or empty), $2 = dst tmp path
  # If src is empty / missing, leave dst empty.
  local src="$1" dst="$2"
  if [[ -n "$src" && -f "$src" ]]; then
    cp "$src" "$dst"
  else
    : > "$dst"
  fi
}

backup_existing() {
  local stamp="$1"
  local backup="$JESSY_DIR/backup-$stamp"
  # Guard against same-second collision by appending $$
  if [[ -d "$backup" ]]; then
    backup="${backup}-$$"
  fi
  mkdir -p "$backup"
  for f in "$CONFIG_FILE" "$PREFS_FILE" "$DB_FILE"; do
    [[ -e "$f" ]] && mv "$f" "$backup/"
  done
  printf 'backed up to %s\n' "$backup" >&2
}

write_config() {
  # $1 = LinkedIn URLs file, $2 = Wellfound URLs file.
  # Keeps the template authoritative, only replacing non-empty startup lists.
  local li_file="$1" wf_file="$2" tmp li_has=0 wf_has=0
  tmp=$(mktemp)
  [[ -s "$li_file" ]] && li_has=1
  [[ -s "$wf_file" ]] && wf_has=1
  awk -v li="$li_file" -v wf="$wf_file" -v li_has="$li_has" -v wf_has="$wf_has" '
    function emit(file,    line) {
      while ((getline line < file) > 0) {
        if (line != "") print "      - " line
      }
      close(file)
    }
    /^  linkedin:/  { platform="linkedin" }
    /^  wellfound:/ { platform="wellfound" }
    /^  [A-Za-z0-9_-]+:/ && $0 !~ /^  (linkedin|wellfound):/ { platform="" }
    skip && /^      - / { next }
    { skip=0 }
    platform=="linkedin" && /^    startup_urls:/ && li_has == "1" {
      print "    startup_urls:"; emit(li); skip=1; next
    }
    platform=="wellfound" && /^    startup_urls:/ && wf_has == "1" {
      print "    startup_urls:"; emit(wf); skip=1; next
    }
    { print }
  ' "$CONFIG_TEMPLATE" > "$tmp"
  mv "$tmp" "$CONFIG_FILE"
}

write_prefs() {
  # $1 = dealbreaker bullets file, $2 = likes bullets file.
  # If a file is empty, the section keeps its placeholder comment.
  local db_file="$1" lk_file="$2"
  local db_empty=1 lk_empty=1
  [[ -s "$db_file" ]] && db_empty=0
  [[ -s "$lk_file" ]] && lk_empty=0
  awk -v db="$db_file" -v lk="$lk_file" -v db_empty="$db_empty" -v lk_empty="$lk_empty" '
    BEGIN { section="" }
    /^## Dealbreakers/ { print; section="db"; next }
    /^## Likes/        { print; section="lk"; next }
    /^## /             { section=""; print; next }
    section=="db" && /^<!--/ {
      if (db_empty == "1") { print; section="done_db"; next }
      while ((getline line < db) > 0) print "- " line
      close(db); section="done_db"; next
    }
    section=="lk" && /^<!--/ {
      if (lk_empty == "1") { print; section="done_lk"; next }
      while ((getline line < lk) > 0) print "- " line
      close(lk); section="done_lk"; next
    }
    { print }
  ' "$PREFS_TEMPLATE" > "$PREFS_FILE"
}

mkdir -p "$JESSY_DIR"

have_config=0; have_prefs=0; have_db=0
[[ -f "$CONFIG_FILE" ]] && have_config=1
[[ -f "$PREFS_FILE" ]] && have_prefs=1
[[ -f "$DB_FILE" ]] && have_db=1

if [[ $FORCE -eq 1 ]]; then
  backup_existing "$(date +%s)"
  have_config=0; have_prefs=0; have_db=0
fi

if [[ $have_config -eq 1 && $have_prefs -eq 1 && $have_db -eq 1 ]]; then
  printf 'jessy already onboarded at %s (use --force to re-prompt)\n' "$JESSY_DIR" >&2
  exit 0
fi

# Capture inputs for missing files
URLS_FILE=$(mktemp); WF_URLS_FILE=$(mktemp); DB_BUL=$(mktemp); LK_BUL=$(mktemp)
trap 'rm -f "$URLS_FILE" "$WF_URLS_FILE" "$DB_BUL" "$LK_BUL"' EXIT

if [[ $NON_INTERACTIVE -eq 1 ]]; then
  [[ $have_config -eq 0 ]] && copy_if_file "$URLS_FILE_ARG" "$URLS_FILE"
  [[ $have_config -eq 0 ]] && copy_if_file "$WF_URLS_FILE_ARG" "$WF_URLS_FILE"
  if [[ $have_prefs -eq 0 ]]; then
    copy_if_file "$DB_FILE_ARG" "$DB_BUL"
    copy_if_file "$LK_FILE_ARG" "$LK_BUL"
  fi
  # Filter startup URLs by platform. Invalid URLs are ignored.
  if [[ -s "$URLS_FILE" ]]; then
    grep -E "^https?://([a-z]+\.)?linkedin\.com/jobs/" "$URLS_FILE" > "${URLS_FILE}.f" || true
    mv "${URLS_FILE}.f" "$URLS_FILE"
  fi
  if [[ -s "$WF_URLS_FILE" ]]; then
    grep -E "^https?://([a-z]+\.)?wellfound\.com/(jobs|role|location|remote)(/|$)" "$WF_URLS_FILE" > "${WF_URLS_FILE}.f" || true
    mv "${WF_URLS_FILE}.f" "$WF_URLS_FILE"
  fi
else
  cat >&2 <<'EOF'

jessy = job scanner. scans tabs, scores vs prefs, ranks.
config in ~/.jessy/. setup takes ~1 min.
EOF
  if [[ $have_config -eq 0 ]]; then
    prompt_lines "LinkedIn search URLs" "^https?://([a-z]+\.)?linkedin\.com/jobs/" > "$URLS_FILE"
    prompt_lines "Wellfound search URLs" "^https?://([a-z]+\.)?wellfound\.com/(jobs|role|location|remote)(/|$)" > "$WF_URLS_FILE"
  fi
  if [[ $have_prefs -eq 0 ]]; then
    prompt_lines "Dealbreakers" "" > "$DB_BUL"
    prompt_lines "Likes"        "" > "$LK_BUL"
  fi
fi

# Write missing files
if [[ $have_config -eq 0 ]]; then
  li_count=0; wf_count=0
  [[ -s "$URLS_FILE" ]] && li_count=$(wc -l < "$URLS_FILE" | tr -d ' ')
  [[ -s "$WF_URLS_FILE" ]] && wf_count=$(wc -l < "$WF_URLS_FILE" | tr -d ' ')
  write_config "$URLS_FILE" "$WF_URLS_FILE"
  printf 'wrote %s (%d LinkedIn url(s), %d Wellfound url(s))\n' "$CONFIG_FILE" "$li_count" "$wf_count" >&2
fi

if [[ $have_prefs -eq 0 ]]; then
  write_prefs "$DB_BUL" "$LK_BUL"
  printf 'wrote %s\n' "$PREFS_FILE" >&2
fi

if [[ $have_db -eq 0 ]]; then
  bash "$DB_SH" init
  printf 'init %s\n' "$DB_FILE" >&2
fi

cat >&2 <<EOF

done. edit later with:
  /jessy:config   (config.yaml)
  /jessy:prefs    (preferences.md)
EOF
