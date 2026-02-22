use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

// LinkedIn-owned DOM extraction logic (selectors, scripts, and parsing).

use super::extraction_engine::{parse_from_value, to_js_array};
use crate::ports::browser::BrowserSession;

pub struct LinkedInSearchSelectors {
    pub card_node_selectors: &'static [&'static str],
    pub title_selectors: &'static [&'static str],
    pub next_page_selectors: &'static [&'static str],
}

pub struct LinkedInJobSelectors {
    pub title_selectors: &'static [&'static str],
    pub company_selectors: &'static [&'static str],
    pub location_selectors: &'static [&'static str],
    pub employment_type_selectors: &'static [&'static str],
    pub posted_text_selectors: &'static [&'static str],
    pub description_selectors: &'static [&'static str],
    pub company_summary_selectors: &'static [&'static str],
    pub company_size_selectors: &'static [&'static str],
}

pub const LINKEDIN_SEARCH_SELECTORS: LinkedInSearchSelectors = LinkedInSearchSelectors {
    card_node_selectors: &[".job-card-container"],
    title_selectors: &[".job-card-container__link > span > strong"],
    next_page_selectors: &[".jobs-search-pagination__button"],
};

pub const LINKEDIN_JOB_SELECTORS: LinkedInJobSelectors = LinkedInJobSelectors {
    title_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
    company_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
    location_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
    employment_type_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
    posted_text_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
    description_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
    company_summary_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
    company_size_selectors: &[
        "[data-sdui-component=\"com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob\"]",
    ],
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPageSnapshot {
    pub page_url: String,
    pub page_title: String,
    #[serde(default)]
    pub job_cards: Vec<SearchCard>,
    #[serde(default)]
    pub job_links: Vec<String>,
    pub next_page_url: Option<String>,
    pub fingerprint_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCard {
    pub title: String,
    pub company_hint: Option<String>,
    pub job_url: String,
    #[serde(default)]
    pub footer_items: Vec<String>,
    #[serde(default)]
    pub posted_datetime: Option<String>,
    pub raw_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetailSnapshot {
    pub url: String,
    pub about_job_dom: String,
    pub title: String,
    pub company: String,
    pub location: Option<String>,
    pub employment_type: Option<String>,
    pub posted_text: Option<String>,
    pub description: String,
    pub requirements: Vec<String>,
    pub company_domain: Option<String>,
    pub company_summary: Option<String>,
    pub company_size: Option<String>,
}

pub async fn extract_search_snapshot(
    session: &mut dyn BrowserSession,
) -> Result<SearchPageSnapshot> {
    session.enable_basics().await?;

    let value = session
        .evaluate(&build_search_extraction_script())
        .await
        .context("failed extracting search page")?;

    parse_search_snapshot(value)
}

pub async fn extract_job_detail_snapshot(
    session: &mut dyn BrowserSession,
) -> Result<JobDetailSnapshot> {
    session.enable_basics().await?;

    let value = session
        .evaluate(&build_job_detail_script())
        .await
        .context("failed extracting job detail")?;

    parse_job_snapshot(value)
}

fn parse_search_snapshot(value: Value) -> Result<SearchPageSnapshot> {
    let mut snapshot: SearchPageSnapshot = parse_from_value(value, "search snapshot parse error")?;

    snapshot
        .job_cards
        .retain(|c| Url::parse(&c.job_url).is_ok());

    if snapshot.job_links.is_empty() {
        snapshot.job_links = snapshot
            .job_cards
            .iter()
            .map(|c| c.job_url.clone())
            .collect::<Vec<_>>();
    }

    snapshot.job_links.retain(|u| Url::parse(u).is_ok());

    Ok(snapshot)
}

fn parse_job_snapshot(value: Value) -> Result<JobDetailSnapshot> {
    let mut snapshot: JobDetailSnapshot = parse_from_value(value, "job snapshot parse error")?;

    if snapshot.company_domain.is_none() {
        snapshot.company_domain = guess_company_domain(&snapshot.url);
    }

    Ok(snapshot)
}

fn guess_company_domain(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|u| u.domain().map(|x| x.to_string()))
}

const SEARCH_EXTRACTION_SCRIPT_TEMPLATE: &str = r#"
(() => {
  const clean = (v) => (v || '').replace(/\s+/g, ' ').trim();
  const CARD_NODE_SELECTORS = __SEARCH_CARD_NODE_SELECTORS__;
  const TITLE_SELECTORS = __SEARCH_TITLE_SELECTORS__;
  const NEXT_PAGE_SELECTORS = __SEARCH_NEXT_PAGE_SELECTORS__;
  const abs = (href) => {
    try { return new URL(href, window.location.href).toString(); } catch (_e) { return null; }
  };
  const titleText = (el) => {
    if (!el) return '';
    const strong = (el.querySelector && el.querySelector('strong')) || null;
    return clean((strong ? strong.innerText : (el.innerText || el.textContent || '')) || '');
  };

  const isJobLike = (href) => {
    const h = (href || '').toLowerCase();
    if (!h) return false;
    if (h.includes('/jobs/search')) return false;
    if (h.includes('/jobs/alerts')) return false;
    return h.includes('linkedin.com/jobs/view')
      || h.includes('/jobs/collections/')
      || h.includes('indeed.com/viewjob')
      || h.includes('indeed.com/rc/clk')
      || h.includes('greenhouse.io')
      || h.includes('lever.co')
      || h.includes('/careers/')
      || h.includes('/jobs/');
  };

  const extractCardFromNode = (node) => {
    if (!node) return null;
    const titleNode = TITLE_SELECTORS
      .map((sel) => node.querySelector(sel))
      .find(Boolean);
    if (!titleNode) return null;
    const jobAnchor = titleNode.closest('a[href]');
    if (!jobAnchor) return null;
    const jobUrl = abs(jobAnchor.getAttribute('href') || '');
    if (!jobUrl || !isJobLike(jobUrl)) return null;
    const title = titleText(titleNode);
    const footerItems = [...node.querySelectorAll('.job-card-container__footer-item')]
      .map((el) => clean(el.innerText || el.textContent || ''))
      .filter(Boolean)
      .slice(0, 8);
    const postedNode = node.querySelector('time[datetime]');
    const postedDatetimeRaw = clean((postedNode && postedNode.getAttribute && postedNode.getAttribute('datetime')) || '');

    return {
      title,
      company_hint: null,
      job_url: jobUrl,
      footer_items: footerItems,
      posted_datetime: postedDatetimeRaw || null,
      raw_text: clean((node.innerText || '').slice(0, 1200)),
    };
  };

  const cardNodes = [];
  for (const sel of CARD_NODE_SELECTORS) {
    cardNodes.push(...document.querySelectorAll(sel));
  }

  const cards = [];
  const seenUrls = new Set();
  for (const node of cardNodes) {
    const c = extractCardFromNode(node);
    if (!c) continue;
    if (seenUrls.has(c.job_url)) continue;
    seenUrls.add(c.job_url);
    cards.push(c);
  }

  const nextCandidates = NEXT_PAGE_SELECTORS
    .flatMap((sel) => [...document.querySelectorAll(sel)]);

  let nextPageUrl = null;
  for (const el of nextCandidates) {
    const label = (el.getAttribute && el.getAttribute('aria-label') || '').toLowerCase();
    if (label.includes('next')) {
      try {
        el.click();
      } catch (_e) {}
      break;
    }
  }

  for (const el of nextCandidates) {
    if (el && el.href) {
      nextPageUrl = abs(el.href || '');
      break;
    }
    if (el && el.closest) {
      const parentA = el.closest('a[href]');
      if (parentA && parentA.href) {
        nextPageUrl = abs(parentA.href);
        break;
      }
    }
  }

  if (!nextPageUrl) {
    try {
      const u = new URL(window.location.href);
      const isLinkedinJobs = /linkedin\.com$/i.test(u.hostname) && u.pathname.includes('/jobs/search');
      if (isLinkedinJobs) {
        const currentStart = Number.parseInt(u.searchParams.get('start') || '0', 10);
        const nextStart = Number.isFinite(currentStart) ? currentStart + 25 : 25;
        u.searchParams.set('start', String(nextStart));
        nextPageUrl = u.toString();
      }
    } catch (_e) {}
  }

  const links = cards.map(c => c.job_url).slice(0, 250);
  const source = `${document.title}|${window.location.href}|${links.slice(0, 40).join('|')}`;

  return {
    page_url: window.location.href,
    page_title: document.title || '',
    job_cards: cards.slice(0, 250),
    job_links: links,
    next_page_url: nextPageUrl,
    fingerprint_source: source,
  };
})();
"#;

const JOB_DETAIL_SCRIPT_TEMPLATE: &str = r#"
(() => {
  const ABOUT_JOB_ROOT_SELECTOR = '[data-sdui-component="com.linkedin.sdui.generated.jobseeker.dsl.impl.aboutTheJob"]';
  const safeText = (v) => (v || '').replace(/\s+/g, ' ').trim();
  const JOB_TITLE_SELECTORS = __JOB_TITLE_SELECTORS__;
  const COMPANY_SELECTORS = __JOB_COMPANY_SELECTORS__;
  const LOCATION_SELECTORS = __JOB_LOCATION_SELECTORS__;
  const EMPLOYMENT_TYPE_SELECTORS = __JOB_EMPLOYMENT_TYPE_SELECTORS__;
  const POSTED_TEXT_SELECTORS = __JOB_POSTED_TEXT_SELECTORS__;
  const DESCRIPTION_SELECTORS = __JOB_DESCRIPTION_SELECTORS__;
  const COMPANY_SUMMARY_SELECTORS = __JOB_COMPANY_SUMMARY_SELECTORS__;
  const COMPANY_SIZE_SELECTORS = __JOB_COMPANY_SIZE_SELECTORS__;

  const collectAllTexts = (sels, opts = {}) => {
    const limit = opts.limit || 120;
    const maxLen = opts.maxLen || 4000;
    const out = [];
    const seen = new Set();
    for (const sel of sels) {
      for (const el of document.querySelectorAll(sel)) {
        const v = safeText(el.innerText || el.textContent || '');
        if (!v || v.length > maxLen) continue;
        const key = v.toLowerCase();
        if (seen.has(key)) continue;
        seen.add(key);
        out.push(v);
        if (out.length >= limit) return out;
      }
    }
    return out;
  };

  const allText = (sels) => collectAllTexts(sels, { limit: 1 })[0] || null;

  const bestByScore = (values, scorer) => {
    let best = '';
    let bestScore = -1000000;
    for (const v of values) {
      const score = scorer(v || '');
      if (score > bestScore) {
        bestScore = score;
        best = v;
      }
    }
    return safeText(best || '');
  };

  const textFromNode = (node) => {
    if (!node) return '';
    return safeText(node.innerText || node.textContent || '');
  };

  const companyTextBlacklist = /(followers?|connections?|employees?|full[- ]?time|part[- ]?time|contract|intern|applicants?|ago|remote|hybrid|onsite|easy apply)/i;
  const cleanCompanyCandidate = (v) => {
    const c = safeText(v || '');
    if (!c || c.length > 140) return '';
    if (companyTextBlacklist.test(c)) return '';
    return c;
  };

  const parseJsonLd = () => {
    const scripts = [...document.querySelectorAll('script[type="application/ld+json"]')];
    for (const s of scripts) {
      try {
        const raw = JSON.parse(s.textContent || '{}');
        const entries = Array.isArray(raw) ? raw : [raw];
        for (const entry of entries) {
          const item = entry['@graph'] ? (Array.isArray(entry['@graph']) ? entry['@graph'] : [entry['@graph']]) : [entry];
          for (const node of item) {
            const t = (node && node['@type']) || '';
            if ((Array.isArray(t) && t.includes('JobPosting')) || t === 'JobPosting') {
              return node;
            }
          }
        }
      } catch (_e) {}
    }
    return null;
  };

  const jobLd = parseJsonLd();
  const ldTitle = safeText(jobLd?.title || '');
  const ldCompany = safeText(jobLd?.hiringOrganization?.name || '');
  const ldEmploymentType = safeText(jobLd?.employmentType || '');
  const ldPosted = safeText(jobLd?.datePosted || '');
  const ldDescription = safeText(
    (jobLd?.description || '').replace(/<[^>]+>/g, ' ')
  );
  const ldLocation = safeText(
    jobLd?.jobLocation?.address?.addressLocality
    || jobLd?.jobLocation?.address?.addressRegion
    || ''
  );

  const scoreTitleCandidate = (v) => {
    const c = safeText(v || '');
    if (!c || c.length < 3 || c.length > 180) return -1000;
    let score = 0;
    if (c.length >= 8 && c.length <= 90) score += 6;
    if (/[A-Za-z]/.test(c)) score += 4;
    if (/^(about the job|about the company|company photos|commitments)$/i.test(c)) score -= 12;
    if (/\b(use ai|show match details|tailor my resume|set alert|apply|save|follow|learn more)\b/i.test(c)) score -= 10;
    if (/\b(hours?|days?|weeks?|months?) ago\b/i.test(c)) score -= 8;
    if (/\bemployees?|followers?|connections?\b/i.test(c)) score -= 8;
    return score;
  };

  const titleCandidates = collectAllTexts(JOB_TITLE_SELECTORS, { limit: 80, maxLen: 220 });
  const titleRaw = bestByScore(
    titleCandidates.concat([ldTitle || '', document.title || '']),
    scoreTitleCandidate
  );

  const findCompanyNearTitle = () => {
    const linkedinCompanyAnchors = [
      ...document.querySelectorAll('a[href^="https://www.linkedin.com/company"], a[href^="http://www.linkedin.com/company"], a[href^="/company/"]')
    ];
    for (const anchor of linkedinCompanyAnchors) {
      const href = (anchor.getAttribute('href') || '').trim();
      if (!(href.startsWith('https://www.linkedin.com/company') || href.startsWith('http://www.linkedin.com/company') || href.startsWith('/company/'))) {
        continue;
      }

      const queue = [...anchor.children];
      while (queue.length > 0) {
        const node = queue.shift();
        const candidate = cleanCompanyCandidate(node?.textContent || '');
        if (candidate) return candidate;
        if (node && node.children && node.children.length) {
          queue.push(...node.children);
        }
      }

      const lineCandidates = (anchor.innerText || anchor.textContent || '')
        .split('\n')
        .map((line) => cleanCompanyCandidate(line))
        .filter(Boolean);
      if (lineCandidates.length > 0) {
        return lineCandidates[0];
      }
    }

    const titleEl = document.querySelector(JOB_TITLE_SELECTORS.join(', '));
    if (!titleEl) return '';

    const cardRoots = [
      titleEl.closest('.job-details-jobs-unified-top-card'),
      titleEl.closest('.jobs-unified-top-card'),
      titleEl.closest('main'),
      document,
    ].filter(Boolean);

    for (const root of cardRoots) {
      const companyNode = root.querySelector(COMPANY_SELECTORS.join(', '));
      const companyText = textFromNode(companyNode);
      if (cleanCompanyCandidate(companyText)) {
        return companyText;
      }
    }

    let cursor = titleEl.previousElementSibling;
    let hops = 0;
    while (cursor && hops < 6) {
      const linkNode = cursor.querySelector('a[href*="linkedin.com/company/"], a[href*="/company/"]');
      const candidate = textFromNode(linkNode || cursor);
      if (cleanCompanyCandidate(candidate)) {
        return candidate;
      }
      cursor = cursor.previousElementSibling;
      hops += 1;
    }

    return '';
  };

  const scoreCompanyCandidate = (v) => {
    const c = cleanCompanyCandidate(v);
    if (!c) return -1000;
    let score = 0;
    if (c.length >= 2 && c.length <= 80) score += 6;
    if (/[A-Za-z]/.test(c)) score += 4;
    if (/\b(linkedin|promoted|apply|save|follow|learn more)\b/i.test(c)) score -= 8;
    return score;
  };
  const companyCandidates = collectAllTexts(COMPANY_SELECTORS, { limit: 120, maxLen: 180 });
  const companyRaw = bestByScore(
    [findCompanyNearTitle()].concat(companyCandidates).concat([ldCompany || '']),
    scoreCompanyCandidate
  );

  const metaLines = collectAllTexts(POSTED_TEXT_SELECTORS, { limit: 80, maxLen: 320 });
  const splitMetaSegments = (lines) => {
    const segs = [];
    for (const line of lines) {
      const parts = line
        .split(/[·•|]/)
        .map((p) => safeText(p))
        .filter(Boolean);
      segs.push(...parts);
    }
    return segs;
  };
  const metaSegments = splitMetaSegments(metaLines);

  const postedPattern = /\b(\d+\s+(minute|hour|day|week|month|year)s?\s+ago|today|yesterday|just now|\d{4}-\d{2}-\d{2})\b/i;
  const employmentPattern = /\b(full[- ]?time|part[- ]?time|contract|intern(ship)?|temporary|freelance|hybrid|remote|on[- ]?site|onsite)\b/i;
  const locationScore = (v) => {
    const c = safeText(v || '');
    if (!c || c.length < 3 || c.length > 140) return -1000;
    if (postedPattern.test(c) || employmentPattern.test(c)) return -1000;
    let score = 0;
    if (c.includes(',')) score += 4;
    if (/\([^)]+\)/.test(c)) score += 2;
    if (/[A-Za-z]/.test(c)) score += 3;
    if (/\bpeople clicked apply|followers?|employees?\b/i.test(c)) score -= 8;
    return score;
  };
  const locationCandidates = metaSegments
    .concat(collectAllTexts(LOCATION_SELECTORS, { limit: 80, maxLen: 180 }))
    .concat([ldLocation || '']);
  const locationRaw = bestByScore(locationCandidates, locationScore);
  const location = locationRaw || null;

  const employmentScore = (v) => {
    const c = safeText(v || '');
    if (!c || c.length > 80) return -1000;
    if (!employmentPattern.test(c)) return -1000;
    let score = 0;
    if (/^(hybrid|remote|on[- ]?site|onsite|full[- ]?time|part[- ]?time|contract|intern(ship)?|temporary|freelance)$/i.test(c)) {
      score += 8;
    }
    if (c.split(' ').length <= 4) score += 3;
    return score;
  };
  const employmentCandidates = metaSegments
    .concat(collectAllTexts(EMPLOYMENT_TYPE_SELECTORS, { limit: 100, maxLen: 100 }))
    .concat([ldEmploymentType || '']);
  const employmentRaw = bestByScore(employmentCandidates, employmentScore);
  const employmentType = employmentRaw || null;

  const postedScore = (v) => {
    const c = safeText(v || '');
    if (!c || c.length > 120) return -1000;
    return postedPattern.test(c) ? 10 : -1000;
  };
  const postedRaw = bestByScore(metaSegments.concat(metaLines).concat([ldPosted || '']), postedScore);
  const postedText = postedRaw || null;

  const descriptionScore = (v) => {
    const c = safeText(v || '');
    if (!c) return -1000;
    let score = 0;
    if (c.length >= 400) score += 10;
    if (c.length >= 1200) score += 10;
    if (/\b(responsibilit|requirement|experience|qualification|about the job)\b/i.test(c)) score += 6;
    if (/\b(show match details|tailor my resume|set alert)\b/i.test(c)) score -= 12;
    return score;
  };
  const descCandidates = collectAllTexts(DESCRIPTION_SELECTORS, { limit: 40, maxLen: 30000 });
  const description = bestByScore(
    descCandidates.concat([ldDescription || '', safeText((document.body?.innerText || '').slice(0, 12000))]),
    descriptionScore
  );

  const companySummaryCandidates = collectAllTexts(COMPANY_SUMMARY_SELECTORS, { limit: 30, maxLen: 10000 });
  const companySummaryRaw = bestByScore(companySummaryCandidates, (v) => {
    const c = safeText(v || '');
    if (!c) return -1000;
    let score = 0;
    if (c.length >= 120) score += 6;
    if (/\b(about the company|founded|industry|employees|mission|we)\b/i.test(c)) score += 4;
    if (/\b(company photos|commitments|learn more)\b/i.test(c)) score -= 6;
    return score;
  });
  const companySummary = companySummaryRaw || null;

  const companySizeTexts = collectAllTexts(COMPANY_SIZE_SELECTORS, { limit: 120, maxLen: 240 });
  const companySizePattern = /\b\d{1,3}(?:,\d{3})?\s*(?:-|to|–)\s*\d{1,3}(?:,\d{3})?\s+employees\b|\b\d[\d,]*\+?\s+employees\b/i;
  let companySize = null;
  for (const t of companySizeTexts.concat([companySummary || '', description || ''])) {
    const m = (t || '').match(companySizePattern);
    if (m && m[0]) {
      companySize = safeText(m[0]);
      break;
    }
  }

  const reqNodes = [...document.querySelectorAll('li, p, span')].map(n => safeText(n.innerText)).filter(Boolean);
  const requirementKeywords = ['require', 'experience', 'must', 'qualification', 'skill', 'proficient', 'knowledge'];
  const requirements = reqNodes
    .filter(line => line.length > 10 && line.length <= 220 && requirementKeywords.some(k => line.toLowerCase().includes(k)))
    .slice(0, 20);

  const parseFromDocTitle = (docTitle) => {
    if (!docTitle) return { title: '', company: '' };
    const cleaned = safeText(docTitle.replace(/\|\s*LinkedIn.*/i, '').replace(/\s*-\s*Indeed.*/i, ''));

    // "Role at Company"
    const atMatch = cleaned.match(/^(.*?)\s+at\s+(.*?)$/i);
    if (atMatch) {
      return { title: safeText(atMatch[1]), company: safeText(atMatch[2]) };
    }

    // "Role - Company - Location"
    const chunks = cleaned.split(' - ').map(x => safeText(x)).filter(Boolean);
    if (chunks.length >= 2) {
      return { title: chunks[0], company: chunks[1] };
    }

    return { title: cleaned, company: '' };
  };

  const inferred = parseFromDocTitle(document.title || '');
  const title = safeText(titleRaw || inferred.title || '');
  const company = safeText(companyRaw || inferred.company || '');

  const companyLink = document.querySelector('a[href*="company" i], a[href*="about" i], a[href*="linkedin.com/company/"]');
  let companyDomain = null;
  if (companyLink && companyLink.href) {
    try {
      companyDomain = new URL(companyLink.href).hostname;
    } catch (_e) {}
  }

  const aboutJobRoot = document.querySelector(ABOUT_JOB_ROOT_SELECTOR);
  const aboutJobDom = aboutJobRoot ? (aboutJobRoot.outerHTML || '') : '';

  return {
    url: window.location.href,
    about_job_dom: aboutJobDom,
    title: title,
    company: company,
    location,
    employment_type: employmentType,
    posted_text: postedText,
    description: description,
    requirements,
    company_domain: companyDomain,
    company_summary: companySummary,
    company_size: companySize,
  };
})();
"#;

fn build_search_extraction_script() -> String {
    SEARCH_EXTRACTION_SCRIPT_TEMPLATE
        .replace(
            "__SEARCH_CARD_NODE_SELECTORS__",
            &to_js_array(LINKEDIN_SEARCH_SELECTORS.card_node_selectors),
        )
        .replace(
            "__SEARCH_TITLE_SELECTORS__",
            &to_js_array(LINKEDIN_SEARCH_SELECTORS.title_selectors),
        )
        .replace(
            "__SEARCH_NEXT_PAGE_SELECTORS__",
            &to_js_array(LINKEDIN_SEARCH_SELECTORS.next_page_selectors),
        )
}

fn build_job_detail_script() -> String {
    JOB_DETAIL_SCRIPT_TEMPLATE
        .replace(
            "__JOB_TITLE_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.title_selectors),
        )
        .replace(
            "__JOB_COMPANY_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.company_selectors),
        )
        .replace(
            "__JOB_LOCATION_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.location_selectors),
        )
        .replace(
            "__JOB_EMPLOYMENT_TYPE_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.employment_type_selectors),
        )
        .replace(
            "__JOB_POSTED_TEXT_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.posted_text_selectors),
        )
        .replace(
            "__JOB_DESCRIPTION_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.description_selectors),
        )
        .replace(
            "__JOB_COMPANY_SUMMARY_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.company_summary_selectors),
        )
        .replace(
            "__JOB_COMPANY_SIZE_SELECTORS__",
            &to_js_array(LINKEDIN_JOB_SELECTORS.company_size_selectors),
        )
}
