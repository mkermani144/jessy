# jessy plugin

Claude Code plugin replacing Rust `jessy`. Drives Chrome via `claude --chrome`,
scans LinkedIn job tabs, scores against user prefs, renders ranked report.

See [`PLAN.md`](PLAN.md) for the v1 design.

## Commands

| Command          | What |
|------------------|------|
| `/jessy:run`     | Full pass: scan + report. |
| `/jessy:scan`    | Scan open LinkedIn tabs, score, persist. Needs `claude --chrome`. |
| `/jessy:report`  | Render ranked cards; pick rows to open in Chrome; mark seen. |
| `/jessy:learn`   | Mine recent open/dismiss patterns; suggest `preferences.md` updates. |
| `/jessy:cleanup` | Prune old / acted-on rows. Never touches unseen rows. |
| `/jessy:config`  | Show path to `~/.jessy/config.yaml`; edit in your own editor. Onboards if missing. |
| `/jessy:prefs`   | Show path to `~/.jessy/preferences.md`; edit in your own editor. Onboards if missing. |
| `/jessy:reset`   | Wipe `~/.jessy` and re-onboard from scratch. Destructive — confirms first, no backup. |

`/jessy:learn` also runs automatically from `/jessy:report` when the
learning cadence is hit (configured in `config.yaml`).

## Install

### Dev (recommended for now)

Pass the plugin directory to Claude Code at launch:

```sh
claude --plugin-dir /absolute/path/to/jessy/plugin
```

In-session reload after editing plugin files: `/reload-plugins`.

Optional shell alias for repeated sessions:

```sh
alias claude-jessy='claude --plugin-dir /absolute/path/to/jessy/plugin --chrome'
```

`--chrome` is needed for `/jessy:scan` and `/jessy:report` — tab open uses
the attached Chrome session.

### Permissions / approval prompts

The plugin ships `plugin/.claude/settings.json` with a `permissions.allow`
list covering the plugin's helper scripts, the small Bash helpers used by
scan/report flows, the Claude-in-Chrome MCP server, the nested
`Skill(jessy-learn)` handoff from report, and `Read` / `Edit` / `Write`
scoped to `~/.jessy/`. With this in place, a normal `/jessy:run` should
not surface per-call approval prompts for plugin internals.

These rules only auto-load when Claude Code reads a `.claude/settings.json`
in scope. If you launch `claude` from a different working directory, copy
or merge the contents of `plugin/.claude/settings.json` into your
`~/.claude/settings.json` (user-level) once. Note: Bash rules use wildcard
prefix matching on the script paths (`Bash(*/scripts/db.sh*)`) so they work
regardless of where the plugin is installed — `${CLAUDE_PLUGIN_ROOT}` does
not expand inside permission rules. One-time user actions still required:
allow the Claude-in-Chrome extension on first `/chrome` use, and attach
the session with `claude --chrome`.

### Optional: bare `/jessy` slash command

Claude Code namespaces plugin commands as `/<plugin>:<cmd>`, so the canonical
full-pass command is `/jessy:run`. If you want `/jessy` (bare) to work too,
install a user-level command once:

```sh
bash /absolute/path/to/jessy/plugin/scripts/install_bare_alias.sh
```

Writes `~/.claude/commands/jessy.md` that delegates to the plugin's skills.
Idempotent; `--force` to overwrite.

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

- **`/jessy:*` commands missing from `/help`**: run `/reload-plugins`. If
  still missing, check `claude --plugin-dir` points at the directory
  containing `.claude-plugin/plugin.json` (not at one level above).
- **`onboard.sh: jq required`** or **`sqlite3 required`**: `brew install jq`
  / `brew install sqlite3`. Both checked up front before any writes.
- **"Interactive prompts don't work"**: onboarding via Claude Code uses
  `--non-interactive` mode driven by AskUserQuestion. Running
  `onboard.sh` directly in a terminal uses stdin prompts.
- **Scan opens 0 tabs**: confirm `claude --chrome` is attached and a
  LinkedIn search/collections tab is open in that browser; or set
  `linkedin.startup_urls` in config and re-run.
- **`db.sh: sqlite3 not on PATH`**: only happens on stripped images;
  `brew install sqlite3`.

## Verification

After `claude --plugin-dir ...`:

### Round 1

1. `/help` lists `/jessy:config`, `/jessy:prefs`, `/jessy:scan`.
2. With `~/.jessy/` absent, `/jessy:config` asks the user (via
   AskUserQuestion) for URLs / dealbreakers / likes, runs
   `onboard.sh --non-interactive`, then prints the config path. Files
   appear at:
   - `~/.jessy/config.yaml`
   - `~/.jessy/preferences.md`
   - `~/.jessy/jessy.db`
3. `sqlite3 ~/.jessy/jessy.db '.schema'` shows `companies`, `jobs`, `meta`
   tables and `jobs_ts`, `jobs_score` indexes.
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
13. `bash plugin/scripts/db.sh seen https://www.linkedin.com/jobs/view/1`
    exits 0; `... view/999` exits 1.
14. `bash plugin/scripts/db.sh count` prints the row count.
15. With `claude --chrome` and a LinkedIn search tab open,
    `/jessy:scan` walks pages, prints `scanned N new; M match; K low; L ignored`,
    and rows appear in `jobs`. Re-running immediately scans 0 new (seen-skip).

### Round 3

16. `bash plugin/scripts/db.sh query_report` emits one JSON object per
    unseen row, sorted by score DESC. `query_report all` includes seen rows.
17. `bash plugin/scripts/db.sh query_report | bash plugin/scripts/render_cards.sh`
    prints box cards + compact lines + ignored tail.
18. `bash plugin/scripts/db.sh mark_action <url> opened` updates the row;
    next `query_report` (default scope `unseen`) excludes it.
19. `/jessy:report` renders, prompts via AskUserQuestion multi-select,
    opens picked URLs in Chrome, marks picks `opened` and rest `dismissed`,
    prints `opened N; dismissed M; unseen 0`.
20. `/jessy:run` runs `/jessy:scan` then `/jessy:report` in one shot.

### Round 4

21. `bash plugin/scripts/db.sh recent_actions 10` emits up to 10 acted-on
    rows JSONL, newest first.
22. `bash plugin/scripts/db.sh cleanup 30 5000` prunes old acted-on rows;
    prints `pruned X; now Y rows`. Unseen rows survive regardless of age.
23. `/jessy:cleanup` runs the skill — same output, reads thresholds from
    `~/.jessy/config.yaml`.
24. After enough acted-on history, `/jessy:report` auto-invokes
    `/jessy:learn` when `jobs_since_last_learn >= cadence[idx]`.
    `meta_get jobs_since_last_learn` resets to `0` and
    `next_cadence_idx` advances by 1 (clamps to last).
25. `/jessy:learn` prompts via AskUserQuestion with candidate patterns,
    appends bullets under the right `preferences.md` section on consent.

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
  commands/
    cleanup.md
    config.md
    run.md
    learn.md
    prefs.md
    report.md
    scan.md
  scripts/
    db.sh
    install_bare_alias.sh
    onboard.sh
    render_cards.sh
    schema.sql
  config/
    config.example.yaml
    preferences.example.md
  PLAN.md
  README.md
```

User data lives at `~/.jessy/{config.yaml, preferences.md, jessy.db}`.
