use super::classify::AiInput;

pub const SYSTEM_PREAMBLE: &str = r#"
You extract structured job-post data from one DOM element string.
Return strict JSON only.
Use factual extraction from the provided DOM content.
Use null for missing scalar fields and empty arrays for missing requirement groups.
Normalize whitespace and deduplicate requirement items.
Use concise keyword-style values for all free-text fields:
- description: compact role keywords/tags only.
- company_summary: compact company keywords/tags only.
- location_text, compensation_text: short keyword phrases.
For visa_policy_text use one of: visa_sponsored, unknown, visa_not_sponsored.
For company_size_text use one of: 1-10, 11-50, 51-500, above.
Keep phrases terse and scannable.
"#;

pub fn build_prompt(input: &AiInput) -> String {
    input.dom_element.clone()
}
