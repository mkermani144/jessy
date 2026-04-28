---
name: linkedin
description: LinkedIn URL patterns and page semantics for jessy job scanning. Use whenever interacting with linkedin.com/jobs/ pages — search results, detail panes, company pages — to read job listings, descriptions, requirements, or company info.
user-invocable: false
---

# LinkedIn platform skill (jessy)

DOM extraction goes through the Claude Chrome extension (`claude --chrome`).
You do not need CSS / ARIA selectors — ask the page in natural terms
("the visible job cards", "the description section", "the requirements heading").
This skill captures the **semantics** of LinkedIn's job pages so you know
*what* to look for, not *how* to query the DOM.

## When this loads

Triggered automatically when the active tab URL contains `linkedin.com/jobs/`
or `linkedin.com/company/` and you are scanning, scoring, or extracting jobs.

## URL patterns

| Pattern                                       | Page kind          |
|-----------------------------------------------|--------------------|
| `linkedin.com/jobs/search/?...`               | Search results     |
| `linkedin.com/jobs/collections/recommended/`  | Recommended list   |
| `linkedin.com/jobs/view/<id>`                 | Detail (full page) |
| `linkedin.com/jobs/search/?currentJobId=<id>` | Detail (right rail)|
| `linkedin.com/company/<slug>/`                | Company page       |

A search result page often shows the detail of the currently-selected job
inline in a right-side rail; the URL gains `currentJobId=<id>`. Treat that
as a detail view — no need to navigate to the standalone `/jobs/view/<id>`.

## Page semantics — search list

- A scrollable left column containing many **job cards**.
- Each card has: title, company name, location, posted-time, sometimes
  an "Easy Apply" badge and a "Promoted" tag.
- Clicking a card loads the detail in the right rail and updates the URL.
- Cards lazy-load on scroll: scroll the list container into view and to
  the bottom to materialize all cards on the current page.
- Pagination: a numbered pager at the bottom of the list, or
  next-page button. Some surfaces use infinite scroll instead.

## Page semantics — detail (rail or full page)

- Header: title, company link, location, posted date, applicant count.
- Body: a long "About the job" / description block.
- Inside the description, look for **requirement** headings — see below.
- Sometimes a "Meet the hiring team" / recruiter card; ignore for v1.

## Requirement headings (extract → `req_hard`)

Match any of these heading variants, case-insensitive, anywhere in the
description body:

- "Qualifications"
- "Requirements"
- "What we're looking for"
- "Must have"
- "Required skills"
- "Basic qualifications"
- "Minimum qualifications"
- "You have"
- "About you"

The bullets / lines under that heading until the next heading = `req_hard`.

## Nice-to-have headings (extract → `req_nice`)

- "Nice to have"
- "Bonus"
- "Bonus points"
- "Preferred qualifications"
- "Preferred"
- "Plus"
- "Would be nice"
- "Pluses"

Bullets / lines under those = `req_nice`. If a single heading mixes both
(e.g. "Qualifications" with sub-list "Required" + "Preferred"), split
accordingly.

## Company info (extract → `companies` row)

On the company page (`linkedin.com/company/<slug>/`):

- About / overview section → `summary` (one short sentence is fine).
- Employee count / company size band → `size`
  (e.g. "11-50", "51-200", "10,001+").
- Industry, headquarters available; not stored in v1.

## Pagination strategy

For each search tab, walk pages up to `linkedin.max_pages` (config).

Stopping rule (**same-list detection**): before opening any card on a new
page, compare the first 3 card URLs against the first 3 URLs from the
previous page. If they match, the pager did not advance — stop walking
that tab.

## Lazy-load handling

- Cards on the search list lazy-load on scroll. Scroll the list to the
  bottom before enumerating, otherwise you miss cards.
- Long descriptions sometimes show a "See more" link — expand it before
  reading, otherwise the requirements section may be truncated.

## Boundary / skip rules (read by jessy-scan)

- Title prefilter: drop cards whose title matches any
  `linkedin.skip_title_keywords` (config) before opening.
- Attempt boundary: if a canonical URL exists in Jessy's attempted state,
  stop lower/older cards for that tab/feed. Do not use LinkedIn `viewed`.
  Canonicalize by stripping query params except `currentJobId`, or use
  `/jobs/view/<id>` as the canonical form.

## Auth + access

User is expected to be logged in via the chrome session. If a card or
detail page redirects to a login wall, surface that to the user and stop —
do not attempt to log in.
