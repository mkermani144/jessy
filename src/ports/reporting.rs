use crate::domain::job::ReportRow;

/// Port for presenting scan output to the user.
pub trait RunReporter: Send + Sync {
    /// Prints report rows for one completed run.
    fn print_report(&self, rows: &[ReportRow], show_not_opportunities: bool);
}
