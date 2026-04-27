# Jessy Skill-Only Plan

Jessy is now a Claude Code skill/plugin repo. The old Rust implementation is
archived under `jessy-old/`.

This plan only tracks confirmed work.

## Goals

- Normal `/jessy:run` should not repeatedly ask for Bash approvals while
  reading/writing `~/.jessy` or running plugin helper scripts.
- Chrome extension stays as-is for now. First use should tell the user to allow
  the extension for upcoming LinkedIn tab read/open actions.
- Report cards should be visible by default, fit the available width, and avoid
  Claude UI collapsing the useful card content.
- Token usage matters more than raw scan speed. Do not add scan strategy
  complexity unless testing proves the current flow is still too slow and the
  change does not materially increase token use.

## Confirmed Work

### 1. Bash Permission Cleanup

Outcome: plugin-internal Bash should not require repeated approval during normal
scan/report/learn/cleanup flows.

Tasks:

- Route internal work through plugin scripts instead of ad hoc shell fragments.
- Make `allowed-tools` match the commands skills actually run.
- Avoid bare `db.sh` references in skill instructions.
- Avoid compound shell snippets in skills when a script can own the operation.
- Keep permission rules scoped to plugin scripts and `~/.jessy` user data.
- Keep Chrome extension permissions out of this item; those are handled by the
  one-time user allow note.

### 2. Batch Report DB Consume

Outcome: report applies user choices with one deterministic DB operation instead
of many one-off `mark_action` calls.

Tasks:

- Add a batch DB command for report consumption.
- Run one SQLite transaction.
- Mark picked URLs as `opened`.
- Mark shown-but-not-picked rows as `dismissed`.
- Mark ignored rows from that report snapshot as `dismissed`.
- Keep one-off `mark_action` for manual/debug use.

### 3. Report Rendering

Outcome: report cards are useful without requiring hidden-output expansion.

Tasks:

- Prevent report output from becoming one huge collapsed Bash block.
- Keep full cards visible for matches.
- Keep low rows compact.
- Keep ignored rows as a count.
- Fit cards to available width.
- Wrap useful fields instead of overflowing.
- Avoid overlong cards; cap field lines where needed.
- Add fixture checks for long titles, long company text, long requirements, and
  long URLs at narrow and normal widths.

### 4. Scan Efficiency

Outcome: reduce wasted shell/DB work and make bottlenecks visible without adding
token-heavy scan strategy changes.

Tasks:

- Batch DB calls where practical, especially seen checks and report-related
  state updates.
- Add timing summary around scan phases.
- Keep current scan semantics until real tests show it is still too slow.
- Prefer lower token usage over skill speed.

Deferred unless testing proves needed:

- scan modes
- hard detail-read caps
- extra company-cache behavior
- broader subagent strategy changes
- CDP/debug-profile browser path
- computer-use browser path

## Done Criteria

- Root remains skill/plugin-only.
- Rust-era code remains under `jessy-old/`.
- Normal report flow uses batch DB consume.
- Report content is visible without hidden expansion for normal-sized runs.
- Scan output includes timing.
