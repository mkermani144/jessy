#!/usr/bin/env bash
# render_cards.sh — format jessy job JSONL into box cards + compact list + tail
# usage: render_cards.sh [--match N] [--low N] [--width N] [--start-index N] < jobs.jsonl

set -euo pipefail

MATCH=70
LOW=30
WIDTH=80
START_INDEX=1

int_or_die() {
  [[ "$1" =~ ^[0-9]+$ ]] || { echo "render_cards.sh: $2 must be non-negative int" >&2; exit 2; }
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --match)       MATCH="$2";       int_or_die "$MATCH"       --match;       shift 2 ;;
    --low)         LOW="$2";         int_or_die "$LOW"         --low;         shift 2 ;;
    --width)       WIDTH="$2";       int_or_die "$WIDTH"       --width;       shift 2 ;;
    --start-index) START_INDEX="$2"; int_or_die "$START_INDEX" --start-index; shift 2 ;;
    -h|--help)
      cat <<EOF
usage: render_cards.sh [--match N] [--low N] [--width N] [--start-index N] < jobs.jsonl

stdin: JSON lines from 'db.sh query_report'.
stdout:
  match (score >= --match, default 70):    box card with [N] pick index
  low   (--low <= score < --match):         compact line with [N] pick index
  ignored (< --low):                        "+N more" tail (no index)
stderr (final line):
  INDEX_MAP\turl1\turl2\t...   (URLs in pick order, tab-separated)
EOF
      exit 0
      ;;
    *) echo "render_cards.sh: unknown arg $1" >&2; exit 2 ;;
  esac
done

command -v jq >/dev/null 2>&1 || {
  echo "render_cards.sh: jq required on PATH (brew install jq)" >&2
  exit 3
}

# Buffer stdin so we can run jq twice (once for output, once for index map).
INPUT="$(cat)"

printf '%s' "$INPUT" | jq -rs \
  --argjson match "$MATCH" \
  --argjson low   "$LOW" \
  --argjson W     "$WIDTH" \
  --argjson S     "$START_INDEX" '
  def trunc($n):
    if   $n <= 1       then "…"
    elif length > $n   then .[0:($n-1)] + "…"
    else .
    end;
  def rpad($n):
    if   $n <= length  then .
    else . + (" " * ($n - length))
    end;

  ($W - 2) as $CW |

  def line($label; $val):
    ($label + ": " + ($val // "")) as $body |
    "│ " + ($body | trunc($CW - 2) | rpad($CW - 2)) + " │";

  def header($idx; $score; $title):
    ("[\($idx)] [MATCH \($score)] \($title)") as $tag |
    ("╭─ " + $tag + " ") as $hp |
    ($W - ($hp | length) - 1) as $n |
    (if $n >= 1
     then $hp + ("─" * $n) + "╮"
     else
       ("╭─ [\($idx)] [MATCH \($score)] ") as $pre |
       ($W - ($pre | length) - 4) as $tn |
       $pre + ($title | trunc($tn)) + " ─╮"
     end);

  def parse_arr($s):
    $s as $x | try ($x | fromjson) catch (
      if ($x // "") == "" then [] else [$x] end
    );

  def card($idx):
    (parse_arr(.req_hard) | join(", ")) as $must |
    (parse_arr(.req_nice) | join(", ")) as $nice |
    ([.company_name, .company_size, .company_summary]
      | map(select(. != null and . != ""))
      | join(" — ")) as $comp |
    [
      header($idx; .score; .title),
      line("Summary"; .desc),
      line("Must";    $must),
      line("Nice";    $nice),
      line("Company"; $comp),
      line("Why";     .rationale),
      line("Link";    .url),
      "╰" + ("─" * ($W - 2)) + "╯"
    ] | join("\n");

  def compact($idx):
    "[\($idx)] • \(.title) @ \(.company_name) — score \(.score) — \(.rationale)  \(.url)"
    | trunc($W);

  . as $all
  | (map(select(.score >= $match)))              as $m
  | (map(select(.score >= $low and .score < $match))) as $l
  | (map(select(.score < $low)) | length)        as $icnt
  | ($m | length) as $mlen

  | [
      ( if $mlen > 0
        then ([range(0; $mlen) as $i | $m[$i] | card($S + $i)] | join("\n\n"))
        else empty end ),
      ( if ($l | length) > 0
        then ([range(0; ($l | length)) as $i | $l[$i] | compact($S + $mlen + $i)] | join("\n"))
        else empty end ),
      ( if $icnt > 0 then "+\($icnt) more non-match jobs ignored" else empty end )
    ]
  | map(select(. != null and . != ""))
  | join("\n\n")
'

# Emit INDEX_MAP on stderr: tab-separated URLs in pick order
# (matches first by score desc as fed in, then low entries; ignored excluded).
INDEX_MAP=$(printf '%s' "$INPUT" | jq -rs \
  --argjson match "$MATCH" \
  --argjson low   "$LOW" '
  . as $all
  | (map(select(.score >= $match)))                   as $m
  | (map(select(.score >= $low and .score < $match))) as $l
  | ([($m + $l)[] | .url]) | join("\t")
')

printf 'INDEX_MAP\t%s\n' "$INDEX_MAP" >&2
