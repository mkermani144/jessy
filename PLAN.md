# PLAN.md

# Jessy Scan Token Burn Fix

## Goal
- Cut Claude token/time burn during LinkedIn scans.
- Preserve card-level context isolation.
- Stop scanning once Jessy reaches already-attempted territory.
- Keep normal scan simple, bounded by discovered novelty, not hard caps.

## Decisions
- No global scan budgets for now.
- No tab cap for now.
- Stop following lower/older cards after first Jessy DB-attempted card in each LinkedIn tab/feed.
- "Seen" means Jessy attempted/seen state, not LinkedIn `viewed`.
- Keep subagents, but use them only as extractors.
- Run extractor subagents serialized, one at a time.
- Extractor gets URL/card only.
- Extractor has no prefs, no rubric, no fit check.
- Main agent owns all matching/ranking.
- Remove lean/full/deepen flow.
- Do not browse company pages during normal scan.
- Do not open extra tabs during extraction.

## Flow
1. Main scan walks each LinkedIn tab/feed top-down.
2. For each card, derive stable job key/url.
3. Check Jessy DB attempt state.
4. If key/url was attempted before:
   - stop lower/older cards for this tab/feed
   - continue next tab/feed
5. For unattempted cards above boundary:
   - persist attempt start
   - call one extractor subagent
   - persist extraction result
6. After extraction batch, main agent scores JSONs against prefs/context.
7. Persist score/result.

## Attempt Boundary
- Any attempted card counts as a boundary:
  - `ok`
  - `partial`
  - `failed`
  - `scored`
  - `accepted`
  - `rejected`
  - `deferred`
- Failed cards still count as normal-scan boundary.
- Failed-card recovery is a separate targeted flow later.
- Normal scan should not dig below attempted territory.

## Extractor Contract
- Input:
  - canonical job URL
  - minimal card fields if already visible
- Output:
  - strict JSON only
  - no markdown
  - no prose outside JSON
  - no fit judgment
  - unknown allowed

## Extractor Schema
```json
{
  "status": "ok",
  "url": "https://...",
  "lang": "en",
  "title": "Staff Backend Engineer",
  "company": "Acme",
  "company_size": "unknown",
  "location": "remote US",
  "seniority": "staff",
  "employment": "full_time",
  "salary": "unknown",
  "visa": "unknown",
  "req": [
    "8 years backend",
    "rust",
    "distributed systems"
  ],
  "nice": [
    "kubernetes"
  ],
  "summary": [
    "Build backend services",
    "Own production systems"
  ],
  "evidence": [
    "Remote - United States",
    "8+ years backend engineering"
  ]
}
```

## Extractor Field Notes
- `status`: extraction health, not job fit.
  - `ok`: useful detail loaded
  - `partial`: some useful detail loaded, key fields missing
  - `failed`: no useful detail
- `location`: include work mode and geography in one field.
  - examples: `remote US`, `on-site Armenia`, `hybrid NYC`, `remote China`
- `req`: requirements and tech together.
- `domain`: omitted. Main agent infers from other fields.
- `bad`: omitted. Depends on prefs, which extractor does not know.
- `lang`: posting language, short code if obvious, else `unknown`.
- `evidence`: short raw snippets for audit. Optional if unavailable.

## Caps
- `req`: max 10
- `nice`: max 5
- `summary`: max 4
- `evidence`: max 4
- Each string: max 120 chars
- No full job description
- No repeated boilerplate

## Enums
- `status`: `ok|partial|failed`
- `seniority`: `intern|junior|mid|senior|staff|principal|exec|unknown`
- `employment`: `full_time|contract|part_time|internship|unknown`

## Failure Policy
- Try once more only for mechanical load issues:
  - `timeout`
  - `load_failed`
  - `detail_not_loaded`
- Do not retry during normal scan for:
  - `auth_wall`
  - `removed`
  - `invalid_url`
  - `not_job`
- Persist final attempt either way.
- Failed attempts count as scan boundary.

## Main Scoring
- Main agent has prefs/context once.
- Main agent scores extracted JSONs.
- Main agent may infer:
  - domain
  - likely role shape
  - fit/misfit
  - uncertainty
- Main decisions:
  - `accept`
  - `maybe`
  - `reject`
  - `defer`

## Remove / Disable
- Per-card judge subagents.
- Parallel stage2 batches.
- Lean/full/deepen modes.
- Per-card prefs/rubric prompt injection.
- Company-page lookup in normal scan.

## Acceptance Checks
- Five LinkedIn tabs do not trigger parallel browser subagents.
- Extractor subagents run one at a time.
- Extractor prompt does not include prefs or scoring rubric.
- First DB-attempted card stops lower/older scanning for that tab/feed.
- LinkedIn `viewed` label is ignored for boundary logic.
- Failed extraction is persisted and counts as attempted.
- Main scoring works from extractor JSON only.
- No normal-scan company-page browsing.
