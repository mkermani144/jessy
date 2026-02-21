use crate::{domain::job::ReportRow, ports::reporting::RunReporter, report::terminal_cards};

/// Terminal renderer adapter for run reports.
pub struct TerminalReporter;

impl RunReporter for TerminalReporter {
    fn print_report(&self, rows: &[ReportRow], show_not_opportunities: bool) {
        terminal_cards::print_report(rows, show_not_opportunities);
    }
}
