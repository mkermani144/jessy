# jessy-scan extractor task

Invoke the platform extractor Agent for exactly one job card. Main thread
handles enumeration, preferences, scoring, and DB writes. The Agent returns
strict JSON only.

## Inputs

- `platform`: `linkedin` or `wellfound`
- `canonical_url`: platform canonical job detail URL
- `card_title`: title from the list
- `card_company_name`: company from the list
- `card_location`: location from the list
- `card_badges`: visible badges/tags from the list
- `card_snippet`: visible preview text from the list

Do not send user preferences or scoring rubric to the Agent.

## Task

1. Choose extractor:
   - `linkedin` -> `jessy-linkedin-extractor`
   - `wellfound` -> `jessy-wellfound-extractor`
2. Open `canonical_url` in the current attached Chrome context.
3. Wait for the job detail body.
4. If description has `See more` or collapsed body text, expand it.
5. Read authoritative title, company, location, seniority, employment type,
   salary, visa, requirements, nice-to-haves, and short responsibility
   summary.
6. Do not open company pages.
7. Do not open extra tabs.
8. Do not judge fit.
9. Do not include full job description or boilerplate.

If a mechanical load issue happens, return `failed` with one of:

- `timeout`
- `load_failed`
- `detail_not_loaded`

If the page is not extractable for a stable reason, return `failed` with one
of:

- `auth_wall`
- `removed`
- `invalid_url`
- `not_job`

## Output

Return exactly one JSON object, no markdown, no prose:

```json
{
  "status": "ok",
  "url": "https://wellfound.com/jobs/123-staff-backend-engineer",
  "lang": "en",
  "title": "Staff Backend Engineer",
  "company": "Acme",
  "company_size": "unknown",
  "location": "remote US",
  "seniority": "staff",
  "employment": "full_time",
  "salary": "unknown",
  "visa": "unknown",
  "req": ["8 years backend", "rust", "distributed systems"],
  "nice": ["kubernetes"],
  "summary": ["Build backend services", "Own production systems"],
  "evidence": ["Remote - United States", "8+ years backend engineering"]
}
```

Failure shape:

```json
{
  "status": "failed",
  "url": "https://wellfound.com/jobs/123-staff-backend-engineer",
  "reason": "detail_not_loaded"
}
```

Rules:

- `url` must equal `canonical_url`.
- `status`: `ok|partial|failed`.
- `company_size`: use a visible detail value if present, else `unknown`.
- `location`: combine work mode and geography, e.g. `remote US`.
- `req`: max 10 strings.
- `nice`: max 5 strings.
- `summary`: max 4 strings.
- `evidence`: max 4 short raw snippets.
- each string: max 120 chars.
- `seniority`: `intern|junior|mid|senior|staff|principal|exec|unknown`.
- `employment`: `full_time|contract|part_time|internship|unknown`.
