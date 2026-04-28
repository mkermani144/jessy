#!/usr/bin/env bash
# report_session.sh - keep report JSON/cards/maps outside Claude chat.
# prepare: snapshot report rows, render cards to a temp file, open less in tmux.
# consume: parse user picks against the saved index map and consume snapshot.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DB_SH="$SCRIPT_DIR/db.sh"
RENDER_SH="$SCRIPT_DIR/render_cards.sh"

: "${JESSY_REPORT_STATE:=${TMPDIR:-/tmp}/jessy-report-state-${UID:-$(id -u)}.tsv}"

usage() {
  cat >&2 <<'EOF'
usage: report_session.sh <prepare|consume> [indices|all|none]

prepare
  Write report snapshot/cards/index files under temp storage, open cards in
  tmux less when available, and print only file paths plus one prompt.

consume <indices|all|none>
  Read the saved snapshot/index map, apply picked URLs via db.sh
  consume_report, and print the final one-line summary only.
EOF
  exit 2
}

shell_quote() {
  # Single-quote a path for tmux's shell command.
  printf "'"
  printf '%s' "$1" | sed "s/'/'\\\\''/g"
  printf "'"
}

config_int() {
  # Read top-level integer config keys, ignoring trailing comments.
  local key="$1" default="$2" config="${JESSY_CONFIG:-$HOME/.jessy/config.yaml}"
  local val=""
  if [[ -f "$config" ]]; then
    val="$(
      awk -v key="$key" '
        $1 == key ":" {
          s = $0
          sub(/^[^:]*:[[:space:]]*/, "", s)
          sub(/[[:space:]]*#.*/, "", s)
          gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
          print s
          exit
        }
      ' "$config"
    )"
  fi
  if [[ "$val" =~ ^[0-9]+$ ]]; then
    printf '%s\n' "$val"
  else
    printf '%s\n' "$default"
  fi
}

detect_width() {
  # Match report width to the viewing terminal; keep fixtures readable.
  local width=""
  if [[ "${JESSY_REPORT_WIDTH:-}" =~ ^[0-9]+$ ]]; then
    width="$JESSY_REPORT_WIDTH"
  elif command -v tmux >/dev/null 2>&1; then
    width="$(tmux display-message -p '#{window_width}' 2>/dev/null || true)"
  fi
  if [[ ! "$width" =~ ^[0-9]+$ ]] && command -v tput >/dev/null 2>&1; then
    width="$(tput cols 2>/dev/null || true)"
  fi
  [[ "$width" =~ ^[0-9]+$ ]] || width=100
  (( width < 56 )) && width=56
  (( width > 140 )) && width=140
  printf '%s\n' "$width"
}

write_index_tsv() {
  # Convert render_cards' INDEX_MAP stderr line to stable "index<TAB>url" TSV.
  local render_err="$1" index_file="$2" line i
  line="$(awk -F $'\t' '$1 == "INDEX_MAP" { print; exit }' "$render_err" || true)"
  : > "$index_file"
  [[ -n "$line" ]] || return 0

  local parts=()
  IFS=$'\t' read -r -a parts <<< "$line"
  for ((i = 1; i < ${#parts[@]}; i++)); do
    [[ -n "${parts[$i]}" ]] || continue
    printf '%d\t%s\n' "$i" "${parts[$i]}" >> "$index_file"
  done
}

open_report_if_possible() {
  # Tests can disable tmux to avoid opening extra windows.
  local cards_file="$1"
  [[ -s "$cards_file" ]] || return 0
  [[ "${JESSY_REPORT_NO_TMUX:-0}" != "1" ]] || return 0
  command -v tmux >/dev/null 2>&1 || return 0
  command -v less >/dev/null 2>&1 || return 0
  tmux display-message -p '#S' >/dev/null 2>&1 || return 0

  tmux new-window -n jessy-report "less -R $(shell_quote "$cards_file")" \
    >/dev/null 2>&1 || true
}

cmd_prepare() {
  local match low width session_dir snapshot cards index render_err state_tmp prompt
  match="$(config_int threshold_match 70)"
  low="$(config_int threshold_low_show 30)"
  width="$(detect_width)"

  session_dir="$(mktemp -d "${TMPDIR:-/tmp}/jessy-report.XXXXXX")"
  snapshot="$session_dir/report.jsonl"
  cards="$session_dir/report.txt"
  index="$session_dir/index.tsv"
  render_err="$session_dir/render.err"

  "$DB_SH" query_report > "$snapshot"
  "$RENDER_SH" --match "$match" --low "$low" --width "$width" < "$snapshot" > "$cards" 2> "$render_err"
  write_index_tsv "$render_err" "$index"
  open_report_if_possible "$cards"

  state_tmp="$(mktemp "${TMPDIR:-/tmp}/jessy-report-state.XXXXXX")"
  {
    printf 'snapshot\t%s\n' "$snapshot"
    printf 'cards\t%s\n' "$cards"
    printf 'index\t%s\n' "$index"
    printf 'db\t%s\n' "${JESSY_DB:-$HOME/.jessy/jessy.db}"
  } > "$state_tmp"
  mv "$state_tmp" "$JESSY_REPORT_STATE"

  if [[ -s "$snapshot" ]]; then
    prompt="Review report outside chat, then reply with indices (e.g. 1,3), all, or none."
  else
    prompt="No unseen jobs; run /jessy:scan first."
  fi

  printf 'snapshot=%s\n' "$snapshot"
  printf 'cards=%s\n' "$cards"
  printf 'index_map=%s\n' "$index"
  printf 'state=%s\n' "$JESSY_REPORT_STATE"
  printf '%s\n' "$prompt"
}

load_state() {
  # Sets globals SNAPSHOT_FILE, INDEX_FILE, DB_FILE from the last prepare.
  SNAPSHOT_FILE=""
  INDEX_FILE=""
  DB_FILE=""
  [[ -f "$JESSY_REPORT_STATE" ]] || {
    echo "report_session.sh: no prepared report state; run prepare first" >&2
    exit 2
  }

  local key value
  while IFS=$'\t' read -r key value; do
    case "$key" in
      snapshot) SNAPSHOT_FILE="$value" ;;
      index) INDEX_FILE="$value" ;;
      db) DB_FILE="$value" ;;
    esac
  done < "$JESSY_REPORT_STATE"

  [[ -f "$SNAPSHOT_FILE" && -f "$INDEX_FILE" ]] || {
    echo "report_session.sh: prepared report files missing; run prepare again" >&2
    exit 2
  }
}

read_index_urls() {
  # Fills URLS where URLS[N] is the URL for displayed index N.
  URLS=("")
  local idx url
  while IFS=$'\t' read -r idx url; do
    [[ "$idx" =~ ^[0-9]+$ && -n "$url" ]] || continue
    URLS[$idx]="$url"
  done < "$INDEX_FILE"
}

append_pick() {
  # Deduplicate picked indices while preserving first-seen order.
  local idx="$1"
  [[ -n "${URLS[$idx]:-}" ]] || return 0
  [[ " $PICKED_INDEXES " == *" $idx "* ]] && return 0
  PICKED_INDEXES+="$idx "
  PICKED_URLS+=("${URLS[$idx]}")
}

parse_picks() {
  # Accept all, none/empty, or comma/space separated numeric indices.
  local raw="$*" lowered compact token normalized i
  lowered="$(printf '%s' "$raw" | tr '[:upper:]' '[:lower:]')"
  compact="$(printf '%s' "$lowered" | tr -d '[:space:]')"
  PICKED_INDEXES=" "
  PICKED_URLS=()

  case "$compact" in
    ""|none) return 0 ;;
    all)
      for ((i = 1; i < ${#URLS[@]}; i++)); do
        append_pick "$i"
      done
      return 0
      ;;
  esac

  normalized="${raw//,/ }"
  for token in $normalized; do
    [[ "$token" =~ ^[0-9]+$ ]] || continue
    append_pick "$token"
  done
}

cmd_consume() {
  load_state
  read_index_urls
  parse_picks "$@"

  if [[ -n "$DB_FILE" ]]; then
    export JESSY_DB="$DB_FILE"
  fi
  "$DB_SH" consume_report "${PICKED_URLS[@]}" < "$SNAPSHOT_FILE"
}

main() {
  local sub="${1:-}"
  [[ -n "$sub" ]] || usage
  shift
  case "$sub" in
    prepare) [[ $# -eq 0 ]] || usage; cmd_prepare ;;
    consume) cmd_consume "$@" ;;
    -h|--help|help) usage ;;
    *) echo "report_session.sh: unknown subcommand: $sub" >&2; usage ;;
  esac
}

main "$@"
