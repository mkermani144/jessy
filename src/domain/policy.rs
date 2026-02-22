use crate::config::FiltersConfig;
use whatlang::detect;

/// Decision returned by lightweight title pre-filtering.
#[derive(Debug, Clone)]
pub struct PreMatchDecision {
    /// If `true`, the scanner may open and parse the detail page.
    pub should_open_detail: bool,
    /// Machine-readable reason used in logs and status traceability.
    pub reason: String,
}

/// Hard exclusion rules applied before any AI call.
///
/// Returns a stable reason key when the job should be rejected immediately.
pub fn hard_exclusion(
    _filters: &FiltersConfig,
    description: &str,
    requirements: &[String],
) -> Option<String> {
    let corpus = format!("{description}\n{}", requirements.join("\n"));
    let normalized = normalize_text(&corpus);

    if has_explicit_no_visa_or_sponsorship(&normalized) {
        return Some("explicit_no_visa_or_sponsorship".to_string());
    }

    if has_explicit_country_residency_requirement(&normalized) {
        return Some("explicit_country_residency_requirement".to_string());
    }

    None
}

/// Fast title-only pre-filter to reduce unnecessary page opens.
pub fn title_pre_match(filters: &FiltersConfig, title: &str) -> PreMatchDecision {
    let normalized_title = normalize_text(title);

    if normalized_title.is_empty() {
        return PreMatchDecision {
            should_open_detail: true,
            reason: "title_missing_open_anyway".to_string(),
        };
    }

    if let Some(reason) = language_pre_match(filters, title) {
        return PreMatchDecision {
            should_open_detail: false,
            reason,
        };
    }

    for blocked in &filters.words_to_avoid_in_title {
        if contains_phrase(&normalized_title, blocked) {
            return PreMatchDecision {
                should_open_detail: false,
                reason: format!("title_avoided:{blocked}"),
            };
        }
    }

    PreMatchDecision {
        should_open_detail: true,
        reason: "title_allowed".to_string(),
    }
}

fn language_pre_match(filters: &FiltersConfig, title: &str) -> Option<String> {
    if filters.allowed_title_languages.is_empty() {
        return None;
    }

    let detected = detect_title_language_code(title);
    if let Some(code) = detected {
        if !filters
            .allowed_title_languages
            .iter()
            .any(|allowed| allowed == &code)
        {
            return Some(format!("title_language_not_allowed:{code}"));
        }
        return None;
    }

    Some("title_language_unknown_not_allowed".to_string())
}

fn detect_title_language_code(title: &str) -> Option<String> {
    let info = detect(title)?;
    Some(format!("{:?}", info.lang()).to_ascii_lowercase())
}

fn normalize_text(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c.is_whitespace() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_phrase(haystack: &str, needle: &str) -> bool {
    let hay = normalize_text(haystack);
    let needle_norm = normalize_text(needle);
    if needle_norm.is_empty() {
        return false;
    }

    let hay_tokens = hay.split_whitespace().collect::<Vec<_>>();
    let needle_tokens = needle_norm.split_whitespace().collect::<Vec<_>>();
    if needle_tokens.len() > hay_tokens.len() {
        return false;
    }

    hay_tokens
        .windows(needle_tokens.len())
        .any(|window| window == needle_tokens.as_slice())
}

fn has_explicit_no_visa_or_sponsorship(text: &str) -> bool {
    let allow_patterns = [
        "visa sponsorship available",
        "sponsorship available",
        "we sponsor visas",
        "we sponsor visa",
        "can sponsor visa",
        "can provide visa sponsorship",
        "visa transfer available",
    ];

    let hard_block_patterns = [
        "no visa sponsorship",
        "without visa sponsorship",
        "unable to sponsor visa",
        "cannot sponsor visa",
        "cant sponsor visa",
        "do not sponsor visa",
        "we do not sponsor visa",
        "not sponsor visa",
        "no sponsorship for visa",
        "must have unrestricted work authorization",
        "must have existing work authorization",
        "must already have work authorization",
        "require unrestricted work authorization",
        "requires unrestricted work authorization",
        "requires existing work authorization",
        "must be legally authorized to work",
        "must already be authorized to work",
    ];

    if allow_patterns.iter().any(|p| text.contains(p)) {
        // Explicit sponsorship availability should not be blocked.
        return false;
    }

    hard_block_patterns.iter().any(|p| text.contains(p))
}

fn has_explicit_country_residency_requirement(text: &str) -> bool {
    let patterns = [
        "must be located in",
        "must be based in",
        "must reside in",
        "must be resident in",
        "must currently reside in",
        "must currently be located in",
        "only candidates located in",
        "only candidates based in",
        "only applicants located in",
        "only applicants based in",
    ];

    let geo_hints = [
        "country",
        "united states",
        "usa",
        "us ",
        "u s ",
        "uk",
        "united kingdom",
        "canada",
        "europe",
        "netherlands",
        "germany",
        "france",
        "spain",
        "italy",
        "switzerland",
        "denmark",
        "sweden",
        "norway",
        "austria",
        "poland",
        "portugal",
        "ireland",
        "australia",
        "new zealand",
        "singapore",
        "japan",
        "india",
        "uae",
    ];

    patterns
        .iter()
        .any(|p| text.contains(p) && geo_hints.iter().any(|g| text.contains(g)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_title_with_avoided_word() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec!["intern".to_string()],
            allowed_title_languages: vec![],
            recent_posted_within_hours: 24,
        };

        let decision = title_pre_match(&filters, "Software Engineer Intern");
        assert!(!decision.should_open_detail);
    }

    #[test]
    fn hard_excludes_explicit_no_visa() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec![],
            allowed_title_languages: vec![],
            recent_posted_within_hours: 24,
        };

        let reason = hard_exclusion(
            &filters,
            "Applicants must already be authorized to work. No visa sponsorship.",
            &[],
        );
        assert_eq!(reason.as_deref(), Some("explicit_no_visa_or_sponsorship"));
    }

    #[test]
    fn does_not_exclude_when_visa_sponsored() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec![],
            allowed_title_languages: vec![],
            recent_posted_within_hours: 24,
        };

        let reason = hard_exclusion(
            &filters,
            "Visa sponsorship available for qualified candidates.",
            &[],
        );
        assert!(reason.is_none());
    }

    #[test]
    fn does_not_block_partial_word_match_in_title() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec!["intern".to_string()],
            allowed_title_languages: vec![],
            recent_posted_within_hours: 24,
        };

        let decision = title_pre_match(&filters, "International Sales Manager");
        assert!(decision.should_open_detail);
    }

    #[test]
    fn blocks_multi_word_phrase_when_words_are_contiguous() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec!["senior engineer".to_string()],
            allowed_title_languages: vec![],
            recent_posted_within_hours: 24,
        };

        let decision = title_pre_match(&filters, "Lead Senior Engineer Platform");
        assert!(!decision.should_open_detail);
    }

    #[test]
    fn blocks_language_when_not_in_allowed_list() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec![],
            allowed_title_languages: vec!["eng".to_string()],
            recent_posted_within_hours: 24,
        };

        let decision = title_pre_match(
            &filters,
            "Senior Softwareentwickler für Plattform und Daten mit Erfahrung in verteilten Systemen",
        );
        assert!(!decision.should_open_detail);
        assert!(decision.reason.starts_with("title_language_not_allowed:"));
    }

    #[test]
    fn allows_language_when_in_allowed_list() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec![],
            allowed_title_languages: vec!["eng".to_string()],
            recent_posted_within_hours: 24,
        };

        let decision = title_pre_match(
            &filters,
            "Senior software engineer with experience in distributed systems and backend services",
        );
        assert!(decision.should_open_detail);
    }

    #[test]
    fn blocks_unknown_language_when_whitelist_is_enabled() {
        let filters = FiltersConfig {
            words_to_avoid_in_title: vec![],
            allowed_title_languages: vec!["eng".to_string()],
            recent_posted_within_hours: 24,
        };

        let decision = title_pre_match(&filters, "//// ???? 12345");
        assert!(!decision.should_open_detail);
        assert_eq!(decision.reason, "title_language_unknown_not_allowed");
    }
}
