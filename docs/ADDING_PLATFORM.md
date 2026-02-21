# Adding a New Platform

This guide shows how to add a new platform (for example Indeed) while keeping hexagonal boundaries.

## Design rules

- Keep orchestration flow unchanged.
- Add platform-specific behavior only in platform adapter and extraction helpers/selectors.
- Keep selector constants explicit and easy to swap as DOM changes.
- Extract from DOM first; normalize later.

## Step-by-step

## 1) Extend platform enum

File: `src/domain/job.rs`

- Add new enum variant to `PlatformKind`.
- Update `PlatformKind::as_str()`.

## 2) Implement adapter

Create file: `src/adapters/platforms/<platform>.rs`

Implement:
- `SearchPageExtractor::extract_search()`
- `JobPageExtractor::extract_job_detail()`
- `PlatformAdapter::{kind,matches_url,is_search_page}`

Input/output contracts are already defined by ports:
- Search: `SearchPageData`
- Detail: `JobDetailData`

## 3) Register adapter

File: `src/adapters/platforms/registry.rs`

- Add adapter instance in `new_default()`.

## 4) Add DOM extraction logic

Recommended approach:
- Create a platform-owned extraction module:
  - `src/adapters/platforms/<platform>_extract.rs`
- Keep selectors/scripts/parsing in that platform module.
- Reuse shared plumbing from `src/adapters/platforms/extraction_engine.rs` for:
  - selector array serialization,
  - JSON parsing helpers.

Requirements for extraction output:
- Search page must produce:
  - enough `job_cards` or fallback `job_links`.
  - `next_page_url` when paging is possible.
  - stable `fingerprint_source` for repeat detection.
- Job detail must produce:
  - `about_job_dom` (required by AI extractor).
  - title, company, description, requirements.
  - optional location, employment type, posted text.
  - optional company summary and size if present.

## 5) Normalize and canonicalize

If URL shape differs:
- Update canonicalization rules in `src/extract/job_page.rs`.
- Keep dedupe key stable over superficial URL params.

## 6) Validate with doctor + scan

Commands:

```bash
cargo run -- doctor --config config/profile.yaml
cargo run -- scan --config config/profile.yaml
```

Check logs for:
- candidate tabs found.
- search pages extracted.
- seeds created.
- detail extraction success.

## Selector strategy (important)

- Define one clear primary selector per field in your platform constants, then expand to fallback selectors only when needed.
- Prefer semantic/attribute anchors over generated class names.
- Keep fallback logic (for example anchor-based link pass).
- Do not rely on exact class hashes.
- Keep selectors/scripts platform-local; do not place platform-specific DOM logic under `src/chrome/*`.

## Minimum acceptance checklist

- Platform tab is discovered by URL matching.
- At least one search page yields job seeds.
- Detail extraction gets non-empty `about_job_dom` and usable fields in most cases.
- Pagination works or degrades gracefully.
- No compile warnings/errors.
- Existing tests pass.
