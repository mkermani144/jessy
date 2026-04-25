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
3. Read:
   - `title` — the detail header title (authoritative; may differ
     slightly from `card_title`).
   - `company_name` — company link text in the detail header.
   - `desc` — the full description body, trimmed. Keep it concise
     (≤ ~1500 chars) — drop boilerplate / equal opportunity / benefits
     fluff.
   - `req_hard` — bullets under the requirement headings listed in the
     `linkedin` skill. JSON array of short strings.
   - `req_nice` — bullets under the nice-to-have headings. JSON array.
4. **Company page** — only if `company_already_known` is `false` AND a
   company link is present:
   - Open the company page in a new tab.
   - Read `company_size` (e.g. "11-50") and a one-sentence
     `company_summary`.
   - Close that tab.
   Otherwise leave both as empty strings.
5. **Score** per the rubric. Build a one-line `rationale` (≤ ~100 chars).
6. Return **exactly one JSON line** on stdout — no prose, no code fence.

## Output JSON shape (single line, all keys required)

```
{"url":"<canonical_url>","title":"<str>","company_name":"<str>","company_size":"<str>","company_summary":"<str>","desc":"<str>","req_hard":["..."],"req_nice":["..."],"score":<int 0-100>,"rationale":"<str>"}
```

Rules:

- `url` must equal the input `canonical_url` verbatim.
- `req_hard` / `req_nice` are JSON arrays of strings (use `[]` if none).
- `score` is an integer in `[0, 100]`.
- If you cannot load the detail after a couple retries, return:
  `{"url":"<canonical_url>","error":"detail_load_failed"}`
  (main thread will skip without writing a row).
- If the page redirects to a login wall, return:
  `{"url":"<canonical_url>","error":"login_wall"}`

Do NOT call `db.sh` yourself — the main thread owns all DB writes.
