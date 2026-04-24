#!/usr/bin/env bash
# render_cards.sh — format jessy job JSONL into box cards + compact list + tail
# usage: render_cards.sh [--match N] [--low N] [--width N] < jobs.jsonl

set -euo pipefail

MATCH=70
LOW=30
WIDTH=80

int_or_die() {
  [[ "$1" =~ ^[0-9]+$ ]] || { echo "render_cards.sh: $2 must be non-negative int" >&2; exit 2; }
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --match) MATCH="$2"; int_or_die "$MATCH" --match; shift 2 ;;
    --low)   LOW="$2";   int_or_die "$LOW"   --low;   shift 2 ;;
    --width) WIDTH="$2"; int_or_die "$WIDTH" --width; shift 2 ;;
    -h|--help)
      cat <<EOF
usage: render_cards.sh [--match N] [--low N] [--width N] < jobs.jsonl

stdin: JSON lines from 'db.sh query_report'.
stdout:
  match (score >= --match, default 70):    box card
  low   (--low <= score < --match):         compact line
  ignored (< --low):                        "+N more" tail
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

jq -rs \
  --argjson match "$MATCH" \
  --argjson low   "$LOW" \
  --argjson W     "$WIDTH" '
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

  def header($score; $title):
    ("[MATCH \($score)] \($title)") as $tag |
    ("╭─ " + $tag + " ") as $hp |
    ($W - ($hp | length) - 1) as $n |
    (if $n >= 1
     then $hp + ("─" * $n) + "╮"
     else
       ("╭─ [MATCH \($score)] ") as $pre |
       ($W - ($pre | length) - 4) as $tn |
       $pre + ($title | trunc($tn)) + " ─╮"
     end);

  def parse_arr($s):
    $s as $x | try ($x | fromjson) catch (
      if ($x // "") == "" then [] else [$x] end
    );

  def card:
    (parse_arr(.req_hard) | join(", ")) as $must |
    (parse_arr(.req_nice) | join(", ")) as $nice |
    ([.company_name, .company_size, .company_summary]
      | map(select(. != null and . != ""))
      | join(" — ")) as $comp |
    [
      header(.score; .title),
      line("Summary"; .desc),
      line("Must";    $must),
      line("Nice";    $nice),
      line("Company"; $comp),
      line("Why";     .rationale),
      line("Link";    .url),
      "╰" + ("─" * ($W - 2)) + "╯"
    ] | join("\n");

  def compact:
    "• \(.title) @ \(.company_name) — score \(.score) — \(.rationale)  \(.url)"
    | trunc($W);

  . as $all
  | (map(select(.score >= $match)))              as $m
  | (map(select(.score >= $low and .score < $match))) as $l
  | (map(select(.score < $low)) | length)        as $icnt

  | [
      ( if ($m | length) > 0 then ([$m[] | card] | join("\n\n")) else empty end ),
      ( if ($l | length) > 0 then ([$l[] | compact] | join("\n")) else empty end ),
      ( if $icnt > 0 then "+\($icnt) more non-match jobs ignored" else empty end )
    ]
  | map(select(. != null and . != ""))
  | join("\n\n")
'
