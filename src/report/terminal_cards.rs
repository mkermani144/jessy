use std::env;

use crate::domain::job::ReportRow;

const PRIMARY: &str = "\x1b[38;5;39m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

pub fn print_report(rows: &[ReportRow], show_not_opportunities: bool) {
    if rows.is_empty() {
        println!("No jobs found in this run.");
        return;
    }

    let width = card_width();
    let visible_rows = rows
        .iter()
        .filter(|r| show_not_opportunities || r.status == "opportunity")
        .collect::<Vec<_>>();

    if visible_rows.is_empty() {
        println!("No rows to display with current filters.");
        return;
    }

    println!();
    print_banner(width);
    for row in visible_rows {
        print_card(row, width);
    }
}

fn card_width() -> usize {
    let default = 108usize;
    let columns = env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default);

    columns.clamp(92, 124)
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

fn print_card(row: &ReportRow, width: usize) {
    let inner = width - 4;
    let badge = if row.status == "opportunity" {
        "[OPPORTUNITY]"
    } else {
        "[NOT OPPORTUNITY]"
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
