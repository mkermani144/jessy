# jessy-scan card subagent task

You are a subagent invoked by `jessy-scan` to extract and score **one
LinkedIn job card**. The main thread handles enumeration and DB writes.
You handle the expensive per-card DOM work in your own context, then
return one JSON line.

## Inputs (provided in your Task prompt)

- `canonical_url` — `https://www.linkedin.com/jobs/view/<id>` (already
  canonicalized; open this in the attached Chrome tab to load the detail).
- `card_title` — the title shown on the search list (already passed
  prefilters; informational).
- `card_company_name`, `card_location`, `card_badges`, `card_snippet`
  — cheap card metadata already visible before opening detail.
- `route_reason` — one short line from stage 1 explaining why this card
  was routed here.
- `scan_mode` — `lean` or `full`.
- `prefs_text` — full text of `~/.jessy/preferences.md`
  (Dealbreakers / Dislikes / Likes / Notes sections).
- `company_already_known` — `true` or `false`. If `true`, the company
  row already exists in the DB; **do not** open the company page, and
  return empty `company_size` / `company_summary` strings.
- `scoring_rubric` — the rubric block (also in main SKILL.md).

You also have access to the `linkedin` skill (auto-loads on
`linkedin.com/jobs/` URLs); use it for page semantics (heading lists,
"See more", company page layout).

## What you do

1. Open `canonical_url` in the Chrome tab. Wait for the detail body.
2. If a "See more" link is visible inside the description, expand it.
3. Read authoritative `title` and `company_name`.
4. If `scan_mode=lean`:
   - Read only enough to make a cheap but informed decision:
     - concise `desc` (target ≤ ~500 chars)
     - up to ~3 strongest `req_hard`
     - up to ~2 strongest `req_nice`
   - Do **not** open the company page.
   - If the job already looks clearly low / not-match with confidence,
     score it now and return a **final row JSON**.
   - If the job looks promising or still ambiguous, return:
     `{"url":"<canonical_url>","decision":"deepen"}`
     Main thread will re-run you once in `scan_mode=full`.
5. If `scan_mode=full`:
   - Read:
     - `desc` — full useful description body, trimmed. Keep concise
       (≤ ~1500 chars) — drop boilerplate / equal opportunity / benefits
       fluff.
     - `req_hard` — bullets under requirement headings.
     - `req_nice` — bullets under nice-to-have headings.
   - **Company page** — only if `company_already_known` is `false` AND a
     company link is present:
     - Open the company page in a new tab.
     - Read `company_size` and one-sentence `company_summary`.
     - Close that tab.
     Otherwise leave both as empty strings.
   - Score per the rubric. Build one-line `rationale` (≤ ~100 chars).
   - Return a **final row JSON**.
6. Return exactly one JSON line on stdout — no prose, no code fence.

## Output JSON shapes

Final row:
```
{"url":"<canonical_url>","title":"<str>","company_name":"<str>","company_size":"<str>","company_summary":"<str>","desc":"<str>","req_hard":["..."],"req_nice":["..."],"score":<int 0-100>,"rationale":"<str>"}
```

Lean-mode deepen sentinel:

```
{"url":"<canonical_url>","decision":"deepen"}
```

Rules:

- `url` must equal the input `canonical_url` verbatim.
- `req_hard` / `req_nice` are JSON arrays of strings (use `[]` if none).
- `score` is an integer in `[0, 100]`.
- In `scan_mode=lean`, bias toward `deepen` on uncertainty. Finalize
  only when the job already looks clearly low / not-match from early
  evidence.
- If you cannot load the detail after a couple retries, return:
  `{"url":"<canonical_url>","error":"detail_load_failed"}`
  (main thread will skip without writing a row).
- If the page redirects to a login wall, return:
  `{"url":"<canonical_url>","error":"login_wall"}`

Do NOT call `db.sh` yourself — the main thread owns all DB writes.
