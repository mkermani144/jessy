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
  # Display-safe-ish string helpers. Inputs are plain text/URLs from job JSON.
  def str: (. // "") | tostring;
  def trunc($n):
    if   $n <= 1       then "…"
    elif length > $n   then .[0:($n-1)] + "…"
    else .
    end;
  def rpad($n):
    if   $n <= length  then .
    else . + (" " * ($n - length))
    end;

  ($W - 4) as $IW |

  # Split long unbroken tokens, then greedily wrap by spaces.
  def token_parts($w):
    if ($w <= 1) then [trunc(1)]
    elif length <= $w then [.]
    else [range(0; length; ($w - 1)) as $i
      | .[$i:($i + ($w - 1))]
        + (if ($i + ($w - 1)) < length then "…" else "" end)]
    end;

  def wrap_words($w):
    if ($w <= 1) then [trunc(1)]
    else
      (str | gsub("[[:space:]]+"; " ") | split(" ")
        | map(select(. != ""))
        | map(token_parts($w))
        | add // []) as $tokens
      | if ($tokens | length) == 0 then [""]
        else reduce $tokens[] as $word ([];
          if length == 0 then [$word]
          else .[-1] as $last
            | if (($last | length) + 1 + ($word | length)) <= $w
              then .[0:length-1] + [$last + " " + $word]
              else . + [$word]
              end
          end)
        end
    end;

  def cap_lines($max; $w):
    if length <= $max then .
    elif $max <= 1 then [((.[0] // "") | trunc($w))]
    else .[0:($max - 1)] + [((.[($max - 1):] | join(" ")) | trunc($w))]
    end;

  def boxed_line($body):
    "│ " + ($body | trunc($IW) | rpad($IW)) + " │";

  def row($label; $val; $max):
    ($label + ": ") as $prefix |
    (" " * ($prefix | length)) as $pad |
    ($IW - ($prefix | length)) as $vw |
    ($val | str | wrap_words($vw) | cap_lines($max; $vw)) as $lines |
    [range(0; ($lines | length)) as $i
      | if $i == 0
        then boxed_line($prefix + $lines[$i])
        else boxed_line($pad + $lines[$i])
        end]
    | join("\n");

  def alt_colors:
    split("\n")
    | [range(0; length) as $i
      | "\u001b[38;5;\(if ($i % 2) == 0 then 255 else 250 end)m\(.[$i])\u001b[0m"]
    | join("\n");

  def header($idx; $score; $title):
    ("[\($idx)] [MATCH \($score)] \($title)") as $tag |
    ("╭─ " + $tag + " ") as $hp |
    ($W - ($hp | length) - 1) as $n |
    (if $n >= 1
     then $hp + ("─" * $n) + "╮"
     else
       ("╭─ [\($idx)] [MATCH \($score)] ") as $pre |
       ($W - ($pre | length) - 4) as $tn |
       $pre + ($title | str | trunc($tn)) + " ─╮"
     end);

  def parse_arr($s):
    $s as $x |
    if ($x | type) == "array" then ($x | map(tostring))
    else (try ($x | fromjson) catch null) as $parsed |
      if ($parsed | type) == "array" then ($parsed | map(tostring))
      elif ($x // "") == "" then []
      elif $parsed != null then [$parsed | tostring]
      else [$x]
      end
    end;

  def card($idx):
    (parse_arr(.req_hard) | join(", ")) as $must |
    (parse_arr(.req_nice) | join(", ")) as $nice |
    (parse_arr(.extract_summary) | join(", ")) as $extract_summary |
    (parse_arr(.evidence) | join(", ")) as $evidence |
    [
      header($idx; .score; .title),
      row("Status";        .extract_status; 1),
      row("Url";           .url;            2),
      row("Lang";          .extract_lang;   1),
      row("Title";         .title;          2),
      row("Company";       .company_name;   2),
      row("Company Size";  .company_size;   1),
      row("Location";      .location;       2),
      row("Seniority";     .seniority;      1),
      row("Employment";    .employment;     1),
      row("Salary";        .salary;         1),
      row("Visa";          .visa;           1),
      row("Req";           $must;           4),
      row("Nice";          $nice;           3),
      row("Summary";       $extract_summary;4),
      row("Evidence";      $evidence;       4),
      row("Company Notes"; .company_summary;3),
      row("Why";           .rationale;      3),
      "╰" + ("─" * ($W - 2)) + "╯"
    ] | join("\n");

  def compact($idx):
    "[\($idx)] low \(.score): \(.title | str) @ \(.company_name | str) — \(.rationale | str)"
    | wrap_words($W)
    | join("\n");

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
  | alt_colors
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
