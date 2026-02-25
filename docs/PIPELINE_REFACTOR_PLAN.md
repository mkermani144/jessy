# Jessy Pipeline Refactor Plan

## Summary
- Refactor current single-crate flow into data pipeline subcommands.
- No orchestration layer. Manual subcommand runs only.
- Keep clear decoupling, trait-heavy ports, tool-agnostic services, hexarch-like boundaries.
- Pipeline order: `extract -> load -> prefilter -> enrich -> serve`.
- Keep old code during migration. Build new wiring in `src-new/`.
- Final cutover only after explicit step-by-step signoff:
  - `rm src`
  - `mv src-new src`

## Architecture Targets
- Step = crate (root monorepo workspace members).
- `src-new/` is temporary wiring/composition area only.
- Shared core crate (`jessy-core`) for domain types, ports, stage rules, shared errors.
- Step crates:
  - `jessy-extract`
  - `jessy-load`
  - `jessy-prefilter`
  - `jessy-enrich`
  - `jessy-serve`
- Thin CLI composition crate:
  - `jessy-cli` (subcommands only)

## Data/Handoff Contract
- DB-first handoff. Write early to DB, update incrementally.
- Primary handoff table: `jobs`.
- Stage tracking:
  - `current_stage` (single stage marker)
  - `status_meta` (reason/details, e.g. `prefiltered:title_excluded`)
- No stage-events table unless required later by breakage or observability gaps.

## CLI Surface
- New manual subcommands:
  - `jessy extract`
  - `jessy load`
  - `jessy prefilter`
  - `jessy enrich`
  - `jessy serve`
- Keep old commands during migration.
- Old commands must show deprecation warning (no routing through new pipeline internals).

## Serve UX
- Terminal/query CLI only in v1.
- `fzf`-style selection/query.
- Shell out to `fzf` when installed.
- Fallback path when `fzf` is missing.

## Delivery Loop (per step)
1. Plan slice.
2. Implement core/types layer.
3. Commit.
4. Verify together.
5. Implement adapters/services layer.
6. Commit.
7. Verify together.
8. Implement CLI/docs/tests layer.
9. Commit.
10. Verify together.

## Commit Strategy
- Commit per layer (not one giant commit, not one commit per full step).

## Verification Standard
- `cargo check --workspace`.
- Relevant unit tests for touched crates.
- Manual smoke run for the current step.
- Repeat before moving to next step.

## Parallel Implementation Model (You + Multiple Agents)
- You run multiple agents concurrently (not Codex subagents).
- Recommended lane split after core/migration baseline:
  - Lane A: `extract`
  - Lane B: `prefilter`
  - Lane C: `serve`
- Dependency lane:
  - `load` baseline first
  - `enrich` after `load` core interface stabilized
- Merge gates:
  - shared core/schema merged first
  - each lane rebases on latest core before merge
  - enforce workspace build green on each merge

## Step Order
1. Workspace + shared core + DB schema updates.
2. `extract`.
3. `load`.
4. `prefilter`.
5. `enrich`.
6. `serve`.
7. Full CLI wiring in `src-new/`.
8. Deprecation warnings for old commands.
9. Final manual pipeline smoke.
10. Your signoff.
11. Cutover (`src-new` -> `src`).

## Notes
- Hexarch reference checked from `../epis`:
  - domain + ports + inbound + outbound separation.
  - trait-first boundaries.
