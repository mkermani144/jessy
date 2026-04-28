# Jessy Usage Reduction Plan

## Context
- Current high usage is not plugin prompt bloat.
- `/context` showed small fixed context:
  - skills: ~1.8k
  - custom agents: ~85
  - MCP tool definitions: ~1.6k
- Main cost is message history and repeated work:
  - Opus/xhigh main thread
  - Chrome/browser reads
  - per-job extractor subagent calls
  - report JSON/cards printed into chat

## Goal
- Cut Jessy scan/report limit usage.
- Keep current scan architecture.
- Keep extractor subagent.
- Do not add split scan modes.

## Decisions
- Use cheaper main models for Jessy skills.
- Keep extractor on Haiku low.
- Cap scan novelty per run.
- Render reports outside Claude chat.
- Keep all large report artifacts in files/tmux.
- Avoid full page/card text in parent context.
- Prefer script/helper output over model output, but do not print bulky helper output into chat.

## 1. Skill Model Frontmatter
Add model/effort to plugin skills.

Targets:

```yaml
# plugin/skills/jessy-scan/SKILL.md
model: sonnet
effort: low
```

```yaml
# plugin/skills/jessy-report/SKILL.md
model: haiku
effort: low
```

```yaml
# plugin/skills/jessy-learn/SKILL.md
model: sonnet
effort: medium
```

Keep:

```yaml
# plugin/agents/jessy-linkedin-extractor.md
model: haiku
effort: low
```

Reason:
- User default may be Opus/xhigh.
- Scan/report are tool discipline + simple scoring, not frontier reasoning.
- Learn can use Sonnet because it interprets preference patterns.

## 2. Scan Cap
Add config:

```yaml
linkedin:
  max_new_per_run: 20
```

Behavior:
- Count every newly attempted unattempted card.
- Includes skipped, failed, partial, scored.
- Stop scan when cap reached.
- Print cap in summary when hit:
  - `scanned N new; M match; K low; L ignored; cap hit`

Reason:
- Prevent runaway first scans.
- Bound subagent count and Chrome reads.

## 3. Report Outside Chat
Current report risk:
- `db.sh query_report` emits JSONL for unseen jobs.
- `render_cards.sh` renders cards.
- If stdout/stderr returns to Claude, all of it enters context.

New report flow:
1. Query report to temp JSONL file.
2. Render cards to temp text file.
3. Write index map to temp TSV/file.
4. Open report in tmux:

```sh
tmux new-window -n jessy-report 'less -R /tmp/jessy-report.txt'
```

5. User replies with indices in chat.
6. Helper consumes picks by reading index file, not by sending URL map through Claude.

Needed helper:
- `plugin/scripts/report_session.sh`
- subcommands:
  - `prepare`
  - `consume <indices|all|none>`

Output to Claude:
- temp paths
- one-line prompt
- final consume summary

Do not print:
- full JSONL
- rendered cards
- full index map

Reason:
- Script-generated cards still count if printed into transcript.
- tmux/less lets user see cards without loading them into model context.

## 4. Compact Scan Parent Context
Keep parent thread compact:
- Extractor final answer strict compact JSON only.
- No full job descriptions in extractor output.
- No verbose per-card assistant narration.
- Batch visible-card attempt checks with `attempted_many`.
- Use DB helpers whose stdout is one-line or empty.

Scan DB outputs are already mostly small:
- `attempted`: `yes|no`
- `attempt_start`: `inserted|skipped`
- `score_job`: `inserted|skipped`
- `count`: integer

Large DB outputs to avoid in chat:
- `query_report`
- `recent_actions`

## 5. Stronger Cheap Prefilter
Before spawning extractor:
- Apply title skip keywords.
- Apply title-only dealbreakers.
- Add optional company/location/snippet skip rules if useful.

Reason:
- Every avoided extractor saves one Haiku request plus Chrome job-detail reads.

## Implementation Order
1. Add skill model/effort frontmatter.
2. Add report temp-file/tmux flow.
3. Add `linkedin.max_new_per_run`.
4. Tighten scan transcript discipline.
5. Add more prefilter fields only if still needed.

## Acceptance Checks
- `/jessy:scan` runs on Sonnet low unless user overrides.
- `/jessy:report` runs on Haiku low or Sonnet low.
- Extractor remains Haiku low.
- Report cards appear in tmux/less, not chat.
- Claude chat receives no full report JSONL/card output.
- Scan stops at `linkedin.max_new_per_run`.
- Existing attempted-boundary behavior remains.
- No split scan modes added.
