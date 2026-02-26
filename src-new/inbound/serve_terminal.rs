use std::process::Stdio;

use anyhow::{Context, Result};
use jessy_serve::{ServeChannel, ServeRow, ServeRunOutput};
use tokio::{io::AsyncWriteExt, process::Command};

pub struct TerminalChannel {
    use_fzf: bool,
}

impl TerminalChannel {
    pub const fn new(use_fzf: bool) -> Self {
        Self { use_fzf }
    }
}

impl ServeChannel for TerminalChannel {
    fn publish<'a>(
        &'a self,
        output: &'a ServeRunOutput,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a {
        async move {
            println!("serve total={} matched={}", output.total, output.matched);
            present_rows(&output.rows, self.use_fzf).await
        }
    }
}

pub async fn present_rows(rows: &[ServeRow], use_fzf: bool) -> Result<()> {
    if rows.is_empty() {
        println!("No enriched jobs available.");
        return Ok(());
    }

    print_table(rows);
    if !use_fzf {
        return Ok(());
    }

    if !fzf_available().await {
        println!("fzf not found; printed table only.");
        return Ok(());
    }

    match choose_row(rows).await? {
        Some(row) => print_detail(row),
        None => println!("No row selected."),
    }

    Ok(())
}

async fn fzf_available() -> bool {
    Command::new("fzf")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn choose_row<'a>(rows: &'a [ServeRow]) -> Result<Option<&'a ServeRow>> {
    let mut child = Command::new("fzf")
        .args([
            "--prompt",
            "serve> ",
            "--height",
            "40%",
            "--layout",
            "reverse",
            "--delimiter",
            "\t",
            "--with-nth",
            "2..",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn fzf")?;

    if let Some(mut stdin) = child.stdin.take() {
        for row in rows {
            let line = format!(
                "{}\t{}\t{}\t{}\t{}\n",
                row.id,
                scrub(&row.platform),
                scrub(&row.title),
                scrub(&row.company),
                scrub(&row.canonical_url)
            );
            stdin
                .write_all(line.as_bytes())
                .await
                .context("failed writing rows to fzf stdin")?;
        }
    }

    let output = child
        .wait_with_output()
        .await
        .context("failed waiting for fzf output")?;
    if !output.status.success() {
        return Ok(None);
    }

    let selected = String::from_utf8_lossy(&output.stdout);
    let selected_id = selected
        .lines()
        .next()
        .and_then(|line| line.split('\t').next())
        .and_then(|raw| raw.parse::<i64>().ok());

    Ok(selected_id.and_then(|id| rows.iter().find(|row| row.id == id)))
}

fn print_table(rows: &[ServeRow]) {
    println!(
        "{:<8} {:<12} {:<36} {:<24} URL",
        "ID", "PLATFORM", "TITLE", "COMPANY"
    );
    for row in rows {
        println!(
            "{:<8} {:<12} {:<36} {:<24} {}",
            row.id,
            truncate(&row.platform, 12),
            truncate(&row.title, 36),
            truncate(&row.company, 24),
            row.canonical_url
        );
    }
}

fn print_detail(row: &ServeRow) {
    println!();
    println!("id: {}", row.id);
    println!("platform: {}", row.platform);
    println!("title: {}", row.title);
    println!("company: {}", row.company);
    println!("url: {}", row.canonical_url);
    println!("status_meta: {}", row.status_meta);
    println!("company_summary: {}", row.company_summary);
    println!("description: {}", row.description);
}

fn truncate(input: &str, max_chars: usize) -> String {
    let input = scrub(input);
    if input.chars().count() <= max_chars {
        return input;
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let mut value = input.chars().take(max_chars - 3).collect::<String>();
    value.push_str("...");
    value
}

fn scrub(input: &str) -> String {
    input
        .replace('\t', " ")
        .replace('\n', " ")
        .trim()
        .to_string()
}
