use sha2::{Digest, Sha256};
use url::Url;

use crate::domain::job::JobDetailData;

#[derive(Debug, Clone)]
pub struct JobExtraction {
    pub canonical_url: String,
    pub company: String,
    pub title: String,
    pub location: Option<String>,
    pub employment_type: Option<String>,
    pub posted_text: Option<String>,
    pub description: String,
    pub requirements: Vec<String>,
    pub company_domain: Option<String>,
    pub company_summary: Option<String>,
    pub company_size: Option<String>,
}

pub fn from_snapshot(snapshot: JobDetailData) -> JobExtraction {
    let canonical_url = canonicalize_url(&snapshot.url);

    JobExtraction {
        canonical_url,
        company: normalize_whitespace(&snapshot.company),
        title: normalize_whitespace(&snapshot.title),
        location: snapshot.location.map(|x| normalize_whitespace(&x)),
        employment_type: snapshot.employment_type.map(|x| normalize_whitespace(&x)),
        posted_text: snapshot.posted_text.map(|x| normalize_whitespace(&x)),
        description: normalize_whitespace(&snapshot.description),
        requirements: snapshot
            .requirements
            .into_iter()
            .map(|r| normalize_whitespace(&r))
            .filter(|r| !r.is_empty())
            .collect(),
        company_domain: snapshot.company_domain,
        company_summary: snapshot
            .company_summary
            .map(|x| normalize_whitespace(&x))
            .filter(|x| !x.is_empty()),
        company_size: snapshot
            .company_size
            .map(|x| normalize_whitespace(&x))
            .filter(|x| !x.is_empty()),
    }
}

pub fn canonicalize_url(raw: &str) -> String {
    let parsed = match Url::parse(raw) {
        Ok(url) => url,
        Err(_) => return raw.to_string(),
    };

    if let Some(linkedin_job) = canonical_linkedin_job_view(&parsed) {
        return linkedin_job;
    }

    let mut url = parsed;
    url.set_fragment(None);

    let keep: Vec<(String, String)> = url
        .query_pairs()
        .into_owned()
        .filter(|(k, _)| {
            matches!(
                k.as_str(),
                "jk" | "vjk" | "currentJobId" | "position" | "jobId" | "gh_jid"
            )
        })
        .collect();

    url.query_pairs_mut().clear();
    for (k, v) in keep {
        url.query_pairs_mut().append_pair(&k, &v);
    }

    let mut out = url.to_string();
    if out.ends_with('/') {
        out.pop();
    }
    out
}

fn canonical_linkedin_job_view(url: &Url) -> Option<String> {
    let host = url.host_str()?.to_ascii_lowercase();
    if !host.ends_with("linkedin.com") {
        return None;
    }

    let mut segments = url.path_segments()?;
    if segments.next()? != "jobs" || segments.next()? != "view" {
        return None;
    }

    let job_id = segments.next()?.trim_matches('/');
    if job_id.is_empty() || !job_id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    Some(format!("https://www.linkedin.com/jobs/view/{job_id}"))
}

pub fn dedupe_key(canonical_url: &str, company: &str, title: &str) -> String {
    let normalized = format!(
        "{}|{}|{}",
        canonical_url.trim().to_ascii_lowercase(),
        normalize_whitespace(company).to_ascii_lowercase(),
        normalize_whitespace(title).to_ascii_lowercase()
    );

    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_url() {
        let url = "https://www.linkedin.com/jobs/view/123/?utm_source=x&currentJobId=123#section";
        let got = canonicalize_url(url);
        assert_eq!(got, "https://www.linkedin.com/jobs/view/123");
    }

    #[test]
    fn canonicalizes_linkedin_host_and_query_noise() {
        let url = "https://nl.linkedin.com/jobs/view/4375809535/?eBP=NON_CHARGEABLE_CHANNEL&trackingId=abc";
        let got = canonicalize_url(url);
        assert_eq!(got, "https://www.linkedin.com/jobs/view/4375809535");
    }

    #[test]
    fn stable_dedupe() {
        let a = dedupe_key("https://a.com/jobs/1", "Acme", "Senior Engineer");
        let b = dedupe_key("https://a.com/jobs/1", " acme ", "Senior   Engineer");
        assert_eq!(a, b);
    }
}
