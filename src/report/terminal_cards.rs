use std::env;

use crate::domain::job::ReportRow;

const PRIMARY: &str = "\x1b[38;5;39m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

pub fn print_report(rows: &[ReportRow], show_not_opportunities: bool, table_only: bool) {
    if rows.is_empty() {
        println!("No jobs found in this run.");
        return;
    }

    let width = card_width();
    let card_rows = rows
        .iter()
        .filter(|r| show_not_opportunities || is_accepted_status(&r.status))
        .collect::<Vec<_>>();

    println!();
    print_banner(width);
    print_summary_table(&rows.iter().collect::<Vec<_>>(), width);
    if table_only {
        return;
    }
    if card_rows.is_empty() {
        println!("No rows to display with current card filters.");
        return;
    }
    for row in card_rows {
        print_card(row, width);
    }
}

fn card_width() -> usize {
    let default = 140usize;
    let columns = env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default);

    columns.clamp(110, 180)
}

fn print_banner(width: usize) {
    println!("{}{}╔{}╗{}", PRIMARY, BOLD, "═".repeat(width - 2), RESET);
    println!(
        "{}{}║ {:<inner$} ║{}",
        PRIMARY,
        BOLD,
        "Jessy Daily Job Results",
        RESET,
        inner = width - 4
    );
    println!("{}{}╚{}╝{}", PRIMARY, BOLD, "═".repeat(width - 2), RESET);
}

fn print_summary_table(rows: &[&ReportRow], width: usize) {
    let page_width = "page".len().max(4);
    let status_width = "accepted".len();
    let reason_width = usize::min(42, usize::max(24, width / 4));
    let total_width = width.saturating_sub(2);
    let available = total_width.saturating_sub(page_width + status_width + reason_width + 12);
    let title_width = available / 3;
    let url_width = available.saturating_sub(title_width);

    println!();
    println!(
        "{}{}{:title_w$} | {:url_w$} | {:page_w$} | {:status_w$} | {:reason_w$}{}",
        PRIMARY,
        BOLD,
        "title",
        "url",
        "page",
        "status",
        "reason",
        RESET,
        title_w = title_width,
        url_w = url_width,
        page_w = page_width,
        status_w = status_width,
        reason_w = reason_width
    );
    println!("{}{}{}", PRIMARY, "-".repeat(total_width), RESET);

    for row in rows {
        println!(
            "{:title_w$} | {:url_w$} | {:page_w$} | {:status_w$} | {:reason_w$}",
            truncate_cell(&row.title, title_width),
            truncate_cell(&row.canonical_url, url_width),
            truncate_cell(&row.source_page_index.to_string(), page_width),
            report_status(row),
            truncate_cell(&report_reason(row), reason_width),
            title_w = title_width,
            url_w = url_width,
            page_w = page_width,
            status_w = status_width,
            reason_w = reason_width
        );
    }
}

fn report_status(row: &ReportRow) -> &'static str {
    if is_accepted_status(&row.status) {
        return "accepted";
    }
    if row.status == "failed" || is_failed_row(row) {
        return "failed";
    }
    "rejected"
}

fn report_reason(row: &ReportRow) -> String {
    match report_status(row) {
        "accepted" => String::new(),
        _ => compact_reason(&row.summary),
    }
}

fn compact_reason(summary: &str) -> String {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(reason) = trimmed.strip_prefix("Rejected at prefilter: ") {
        return format!("prefilter:{}", normalize_reason_key(reason));
    }
    if trimmed.eq_ignore_ascii_case("Rejected: already seen in history") {
        return "seen_history".to_string();
    }
    if trimmed.eq_ignore_ascii_case("Rejected: duplicate in current scan") {
        return "duplicate_scan".to_string();
    }
    if let Some(reason) = trimmed.strip_prefix("Rejected: hard exclusion (") {
        let key = reason.trim_end_matches(')').trim();
        return format!("hard_exclusion:{}", normalize_reason_key(key));
    }
    if let Some(reason) = trimmed.strip_prefix("Hard exclusion: ") {
        return format!("hard_exclusion:{}", normalize_reason_key(reason));
    }
    if trimmed
        .to_ascii_lowercase()
        .starts_with("failed extraction/classification:")
    {
        return "extract_failed".to_string();
    }

    normalize_reason_key(trimmed)
}

fn normalize_reason_key(input: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_underscore = false;
            continue;
        }
        if !prev_underscore {
            out.push('_');
            prev_underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}

fn is_accepted_status(status: &str) -> bool {
    status == "opportunity" || status == "accepted"
}

fn is_failed_row(row: &ReportRow) -> bool {
    row.summary
        .to_ascii_lowercase()
        .contains("failed extraction/classification")
}

fn truncate_cell(value: &str, max_chars: usize) -> String {
    let clean = value.replace('\n', " ");
    let len = clean.chars().count();
    if len <= max_chars {
        return clean;
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let mut out = clean.chars().take(max_chars - 3).collect::<String>();
    out.push_str("...");
    out
}

fn print_card(row: &ReportRow, width: usize) {
    let inner = width - 4;
    let badge = match report_status(row) {
        "accepted" => "[ACCEPTED]",
        "failed" => "[FAILED]",
        _ => "[REJECTED]",
    };
    let requirement_keywords = extract_requirement_keywords(&row.requirements);
    let requirements_text = if requirement_keywords.is_empty() {
        "none extracted".to_string()
    } else {
        requirement_keywords.join(", ")
    };

    println!();
    println!("{}╭{}╮{}", PRIMARY, "─".repeat(width - 2), RESET);
    print_wrapped_line(&format!("{badge} {}", row.title), inner);
    println!("{}├{}┤{}", PRIMARY, "─".repeat(width - 2), RESET);

    print_kv_opt("Company", row.company.as_deref(), inner);
    print_kv_opt("Location", row.location.as_deref(), inner);
    print_kv_opt("Language", row.language.as_deref(), inner);
    print_kv_opt("Work Mode", row.work_mode.as_deref(), inner);
    print_kv_opt("Employment Type", row.employment_type.as_deref(), inner);
    print_kv_opt("Posted", row.posted_text.as_deref(), inner);
    print_kv_opt("Compensation", row.compensation_text.as_deref(), inner);
    print_kv_opt("Visa Policy", row.visa_policy_text.as_deref(), inner);
    print_kv("Summary", &row.summary, inner);
    print_kv("Link", &row.canonical_url, inner);
    print_kv("Requirements", &requirements_text, inner);
    print_kv_opt("Description", row.description.as_deref(), inner);
    print_kv_opt("Company Summary", row.company_summary.as_deref(), inner);
    print_kv_opt("Company Size", row.company_size.as_deref(), inner);

    println!("{}╰{}╯{}", PRIMARY, "─".repeat(width - 2), RESET);
}

fn print_kv(label: &str, value: &str, inner: usize) {
    if value.trim().is_empty() {
        print_wrapped_line(&format!("{label}:"), inner);
        return;
    }

    let text = format!("{label}: {value}");
    print_wrapped_line(&text, inner);
}

fn print_kv_opt(label: &str, value: Option<&str>, inner: usize) {
    if let Some(v) = value {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            print_kv(label, trimmed, inner);
        }
    }
}

fn print_wrapped_line(text: &str, inner: usize) {
    let clean = text.replace('\n', " ");
    let lines = wrap_text(&clean, inner);

    for line in lines {
        println!(
            "{}│{} {:<inner$} │{}{}",
            PRIMARY,
            RESET,
            line,
            PRIMARY,
            RESET,
            inner = inner
        );
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut out = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if word.len() > max_width {
            if !current.is_empty() {
                out.push(current);
                current = String::new();
            }

            for chunk in split_long_word(word, max_width) {
                out.push(chunk);
            }
            continue;
        }

        if current.is_empty() {
            current.push_str(word);
            continue;
        }

        if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            out.push(current);
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    if out.is_empty() {
        vec![String::new()]
    } else {
        out
    }
}

fn split_long_word(word: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for ch in word.chars() {
        if current.chars().count() >= max_width {
            chunks.push(current);
            current = String::new();
        }
        current.push(ch);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn extract_requirement_keywords(requirements: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for req in requirements {
        let clean = req.trim();
        if clean.is_empty() {
            continue;
        }
        if !out.iter().any(|v: &String| v.eq_ignore_ascii_case(clean)) {
            out.push(clean.to_string());
        }
    }
    out
}
