# Architecture and Important Files

Jessy follows a hexagonal architecture.

## Layers

- Domain (`src/domain`): pure business models and rules.
- Ports (`src/ports`): interfaces orchestration depends on.
- CLI use-cases (`src/cli_commands/use_cases`): orchestrates flow using ports.
- Adapters (`src/adapters`): concrete implementations of ports.

Rule: use-cases import ports + domain, not concrete infra modules.

## Runtime composition

Composition happens in `src/cli.rs`:
- Build concrete adapters.
- Inject them into `ScanDeps`.
- Execute use-cases.

## Important files by responsibility

## Entry points

- `src/main.rs`: binary bootstrap.
- `src/cli.rs`: command parsing and dependency wiring.

## CLI use-cases

- `src/cli_commands/use_cases/scan.rs`: `scan`, `scan-dev`, `doctor`, `cleanup` orchestration.
- `src/cli_commands/use_cases/chrome.rs`: dedicated debug Chrome startup command.

## Domain

- `src/domain/job.rs`: canonical job models and status.
- `src/domain/ai.rs`: AI input/output contracts.
- `src/domain/policy.rs`: title prefilter + hard exclusion rules.

## Ports

- `src/ports/browser.rs`: browser/tab/session contracts.
- `src/ports/platform.rs`: platform extraction contracts and catalog.
- `src/ports/ai.rs`: AI structured-extraction contract.
- `src/ports/storage.rs`: scan persistence contract.
- `src/ports/reporting.rs`: report rendering contract.

## Adapters

- Browser: `src/adapters/browser/chrome.rs`
- Platform registry: `src/adapters/platforms/registry.rs`
- LinkedIn platform extractor: `src/adapters/platforms/linkedin.rs`
- LinkedIn extraction package:
  - `src/adapters/platforms/linkedin_extract.rs`
- Shared platform extraction plumbing:
  - `src/adapters/platforms/extraction_engine.rs`
- AI adapter (Rig/OpenAI):
  - `src/adapters/ai/openai.rs`
- Storage (SQLite): `src/adapters/storage/sqlite.rs`
- Reporting (terminal cards): `src/adapters/reporting/terminal.rs`

## Infra/helper modules used by adapters

- CDP client and browser helpers:
  - `src/chrome/cdp_client.rs`
  - `src/chrome/launcher.rs`
  - (intentionally no platform-specific selectors/scripts here)
- AI prompt + Rig client:
  - `src/ai/prompts.rs`
  - `src/ai/rig_openai_client.rs`
- Storage internals:
  - `src/store/db.rs`
  - `src/store/queries.rs`
  - `src/store/retention.rs`

## Data and schema

- DB migration: `migrations/001_init.sql`
- Default DB path: `data/jessy.db`

Primary tables:
- `jobs`
- `run_logs`
- `run_job_results`
- `job_observations`
- `search_page_fingerprints`

## Configuration

- Example config: `config/profile.example.yaml`
- Default active config: `config/profile.yaml`
- Typed config model: `src/config.rs`
