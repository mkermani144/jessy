---
name: jessy-onboard
description: First-run setup for jessy — ask the user for job search URLs, dealbreakers, and likes via AskUserQuestion, then write ~/.jessy/{config.yaml, preferences.md, jessy.db}. Invoked by every jessy command when ~/.jessy is missing or incomplete.
user-invocable: false
allowed-tools:
  - Bash(test *)
  - Bash(${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh*)
  - Write
  - AskUserQuestion
---

# jessy-onboard

Centralized onboarding flow. Any jessy command / skill that needs
`~/.jessy/config.yaml` + `~/.jessy/preferences.md` calls this first when
either is missing. Idempotent: if both exist, returns immediately.

## Procedure

### 1. Check state

```
test -f ~/.jessy/config.yaml && test -f ~/.jessy/preferences.md
```

If both exist → print nothing; return. Nothing to do.

### 2. Ask the user (AskUserQuestion)

One question at a time, free-text answers. For each, the user can leave
the answer blank to skip.

1. `LinkedIn search URLs you want jessy to scan? (one per line, or blank to skip)`
2. `Wellfound search URLs you want jessy to scan? (one per line, or blank to skip)`
3. `Dealbreakers — jobs matching these auto-score 0 (one per line, or blank)`
4. `Likes — jobs matching these get a score boost (one per line, or blank)`

Collect each answer as a multi-line string.

### 3. Write inputs to temp files

For each non-empty answer, use Write to create a temporary input file
under `/tmp` (one value per line, leading/trailing whitespace trimmed,
empty lines dropped).

If the answer was blank, pass `/dev/null` instead of a temp file path
(script handles empty files fine).

### 4. Run onboarding

```
${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh \
  --non-interactive \
  --urls-file "<urls_file_or_/dev/null>" \
  --wellfound-urls-file "<wellfound_urls_file_or_/dev/null>" \
  --dealbreakers-file "<dealbreakers_file_or_/dev/null>" \
  --likes-file "<likes_file_or_/dev/null>"
```

`${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh` is idempotent — re-runs when
partially onboarded only write the missing files. `--force` is NOT used
here; users who want to reset run
`${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh --force` manually.

### 5. Report back

Print:
```
onboarded — config at ~/.jessy/config.yaml, prefs at ~/.jessy/preferences.md
edit via /jessy:config or /jessy:prefs any time
first scan/report may ask for Chrome extension access; allow it for the upcoming job tab read/open actions
```

Then return control to the caller (do not continue with a scan / report
automatically — each caller decides).

## What this skill does NOT do

- Open an editor (Bash tool has no TTY).
- Launch Chrome, scan, score, or query the DB.
- Validate platform URLs beyond what
  `${CLAUDE_PLUGIN_ROOT}/scripts/onboard.sh` does (HTTP + linkedin.com
  or wellfound.com domain). Invalid URLs are silently dropped with a log line.
