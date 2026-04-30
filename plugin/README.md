# jessy plugin

Claude Code plugin replacing Rust `jessy`. Drives Chrome via `claude --chrome`,
scans LinkedIn / Wellfound job tabs, scores against user prefs, renders ranked
report. The main agent is a supervisor only: browser, judge, report, ops, and
learn workers move payloads through SQLite/temp files and return compact
receipts.

## Commands

| Command          | What |
|------------------|------|
| `/jessy:run`     | Full pass: scan, report, learn/cleanup when due. Needs `claude --chrome`. |
| `/jessy:config`  | Show path to `~/.jessy/config.yaml`; edit in your own editor. Onboards if missing. |
| `/jessy:prefs`   | Show path to `~/.jessy/preferences.md`; edit in your own editor. Onboards if missing. |

Learning and cleanup are internal `/jessy:run` stages, not visible commands.

## Install

### Dev (recommended for now)

Pass the plugin directory to Claude Code at launch:

```sh
claude \
  --settings /absolute/path/to/jessy/plugin/.claude/settings.json \
  --plugin-dir /absolute/path/to/jessy/plugin \
  --chrome
```

In-session reload after editing plugin files: `/reload-plugins`.

Optional shell alias for repeated sessions:

```sh
alias claude-jessy='claude --settings /absolute/path/to/jessy/plugin/.claude/settings.json --plugin-dir /absolute/path/to/jessy/plugin --chrome'
```

`--chrome` is needed for `/jessy:run` — tab read/open
uses the attached Chrome session. On first use, allow the Claude-in-Chrome
extension for the upcoming job tab actions; normal runs assume that access is
already granted.

### Permissions / approval prompts

The plugin ships `plugin/.claude/settings.json` with a `permissions.allow`
list covering helper scripts, stage-bus helpers, scan compound DB helpers,
report-session flow, Claude-in-Chrome MCP tools, internal skill handoffs, and
`Read` / `Edit` / `Write` scoped to `~/.jessy/`.

Important: `--plugin-dir` loads plugin commands/skills, but does **not** load
`plugin/.claude/settings.json` as a settings source. Use `--settings
/absolute/path/to/jessy/plugin/.claude/settings.json`, or merge that file into
`~/.claude/settings.json` once. Without that, `db.sh` and other plugin helpers
can still ask for approval.

Bash rules use wildcard path matching (`Bash(*/scripts/db.sh*)`) so they work
regardless of where the plugin is installed. `${CLAUDE_PLUGIN_ROOT}` is only
for plugin skill/command content, not external settings files. One-time user
actions still required: allow the Claude-in-Chrome extension on first `/chrome`
use, and attach the session with `claude --chrome`.

DB work must be invoked as literal `db.sh` / `db_stage.sh` / `db_scan.sh`
script calls. Do not wrap it in `$DB`, shell functions, or shell loops; those
change the command shape and can trigger approval prompts.

### Marketplace

Not published yet. When it lands, install will be:
`/plugin marketplace add <ref> && /plugin install jessy@<ref>`.
Until then, use `--plugin-dir`.

## Requirements

- macOS (bash 3.2+ supported) or Linux.
- `sqlite3` on PATH (macOS ships it).
- `jq` on PATH (`brew install jq`). Onboarding fails without it.
- Chromium-based browser attached via `claude --chrome` for scan / open.

## Troubleshooting

- **Jessy commands missing from `/help`**: run `/reload-plugins`. If
  still missing, check `claude --plugin-dir` points at the directory
  containing `.claude-plugin/plugin.json` (not at one level above).
- **`onboard.sh: jq required`** or **`sqlite3 required`**: `brew install jq`
  / `brew install sqlite3`. Both checked up front before any writes.
- **"Interactive prompts don't work"**: onboarding via Claude Code uses
  `--non-interactive` mode driven by AskUserQuestion. Running
  `onboard.sh` directly in a terminal uses stdin prompts.
- **Scan opens 0 tabs**: confirm `claude --chrome` is attached and a
  LinkedIn or Wellfound list tab is open in that browser; or set
  `platforms.<name>.startup_urls` in config and re-run.
- **`db.sh: sqlite3 not on PATH`**: only happens on stripped images;
  `brew install sqlite3`.

## Verification

After `claude --plugin-dir ...`:

### Round 1

1. `/help` lists `/jessy:run`, `/jessy:config`, `/jessy:prefs`.
2. With `~/.jessy/` absent, `/jessy:config` asks the user (via
   AskUserQuestion) for URLs / dealbreakers / likes, runs
   `onboard.sh --non-interactive`, then prints the config path. Files
   appear at:
   - `~/.jessy/config.yaml`
   - `~/.jessy/preferences.md`
   - `~/.jessy/jessy.db`
3. `sqlite3 ~/.jessy/jessy.db '.schema'` shows `companies`, `jobs`,
   `job_attempts`, `runs`, `stage_items`, `stage_events`, snapshot tables,
   `meta`, and related indexes.
4. `bash plugin/scripts/db.sh meta_get jobs_since_last_learn` prints `0`.
5. `bash plugin/scripts/db.sh meta_set foo bar && bash plugin/scripts/db.sh meta_get foo`
   prints `bar`.
6. Re-running `bash plugin/scripts/db.sh init` is a no-op (no errors,
   meta values preserved).
7. Re-running `/jessy:config` skips the onboarding AskUserQuestion
   prompts and just prints the path.
8. `bash plugin/scripts/onboard.sh --force` backs up existing files to
   `~/.jessy/backup-<unix-ts>/` (or `backup-<ts>-<pid>/` on same-second
   collision) and writes fresh templates.
9. `/jessy:prefs` never spawns an editor; it prints the path and
   delegates editing to the user.
10. `bash plugin/scripts/db.sh bogus` exits 2 with usage on stderr.

### Round 2

11. `bash plugin/scripts/db.sh upsert_company "Acme" "11-50" "demo"` prints `1`.
    Re-run with empty `size`/`summary` keeps original values.
12. `bash plugin/scripts/db.sh insert_job https://www.linkedin.com/jobs/view/1 1 "T" "D" '[]' '[]' linkedin 50 "r"`
    inserts a row.
13. `bash plugin/scripts/db.sh attempted https://www.linkedin.com/jobs/view/1`
    prints `yes`; `... view/999` prints `no`. `seen` is a backcompat alias.
14. `bash plugin/scripts/db.sh count` prints the row count.
15. With `claude --chrome` and a LinkedIn / Wellfound search tab open,
    `/jessy:run` walks pages, prints `scanned N new; M match; K low; L ignored`,
    then opens the report flow. Re-running immediately scans 0 new because
    the first attempted card stops each tab/feed.

### Round 3

16. `bash plugin/scripts/db.sh query_report` emits one JSON object per
    unseen row, sorted by score DESC. `query_report all` includes seen rows.
17. `bash plugin/scripts/db.sh query_report | bash plugin/scripts/render_cards.sh`
    prints box cards + compact lines + ignored tail for local inspection.
18. `JESSY_REPORT_NO_TMUX=1 bash plugin/scripts/report_session.sh prepare`
    prints only temp paths plus the pick prompt. It does not print cards,
    JSONL, or the full index map into chat.
19. `bash plugin/scripts/report_session.sh consume none` consumes that
    snapshot and prints `opened 0; dismissed M; unseen 0.`.
20. `/jessy:run` opens cards in tmux/less when available, prompts for
    indices in chat, consumes the snapshot, and prints
    `opened N; dismissed M; unseen 0.`.
21. `/jessy:run` is the only scan/report entrypoint.

### Round 4

22. `bash plugin/scripts/db.sh recent_actions 10` emits up to 10 acted-on
    rows JSONL, newest first.
23. `bash plugin/scripts/db.sh cleanup 30 5000` prunes old acted-on rows;
    prints `pruned X; now Y rows`. Unseen rows survive regardless of age.
24. Cleanup runs internally when configured; thresholds come from
    `~/.jessy/config.yaml`.
25. After enough acted-on history, `/jessy:run` invokes learning when
    `jobs_since_last_learn >= cadence[idx]`.
    `meta_get jobs_since_last_learn` resets to `0` and
    `next_cadence_idx` advances by 1 (clamps to last).
26. Learning prompts via AskUserQuestion with candidate patterns,
    appends bullets under the right `preferences.md` section on consent.

### Stage bus

27. `bash plugin/tests/check_stage_bus.sh` passes.
28. `bash plugin/tests/check_context_contracts.sh` passes.
29. `bash plugin/scripts/db_stage.sh run_create` prints a compact JSON
    receipt with `run_id`.
30. Snapshot helpers persist payload text but stdout receipts contain only ids
    and counts.

## Layout

```
plugin/
  .claude-plugin/plugin.json
  skills/
    jessy-cleanup/SKILL.md
    jessy-learn/SKILL.md
    jessy-onboard/SKILL.md
    jessy-report/SKILL.md
    jessy-scan/SKILL.md
    platforms/linkedin/SKILL.md
    platforms/wellfound/SKILL.md
  agents/
    jessy-browser-worker.md
    jessy-judge-worker.md
    jessy-learn-worker.md
    jessy-linkedin-extractor.md
    jessy-ops-worker.md
    jessy-report-worker.md
    jessy-wellfound-extractor.md
  commands/
    config.md
    run.md
    prefs.md
  scripts/
    db.sh
    db_scan.sh
    db_stage.sh
    onboard.sh
    report_session.sh
    render_cards.sh
    schema.sql
  config/
    config.example.yaml
    preferences.example.md
  tests/
    check_context_contracts.sh
    check_db_attempts.sh
    check_report_session.sh
    check_render_cards.sh
    check_stage_bus.sh
  README.md
```

User data lives at `~/.jessy/{config.yaml, preferences.md, jessy.db}`.
