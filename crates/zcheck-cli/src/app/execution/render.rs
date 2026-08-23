//! Human and JSON run-report rendering.

use super::super::args::RunFormat;
use super::super::error::AppError;
use super::model::{RunReport, TaskStatus, status_label};
use zcheck_core::RunStatus;

pub(crate) fn render(report: &RunReport, format: RunFormat) -> Result<String, AppError> {
    match format {
        RunFormat::Json => {
            return report
                .receipt()
                .to_json_pretty()
                .map_err(|error| AppError::internal(error.to_string()));
        }
        RunFormat::Github => return Ok(super::github::render(report)),
        RunFormat::Human => {}
    }
    let selected = report
        .receipt()
        .selection
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec![
        format!("zcheck · {} · {selected}", report.project()),
        String::new(),
    ];
    for task in &report.receipt().tasks {
        let detail = task
            .reason
            .as_deref()
            .map_or_else(String::new, |reason| format!("  {reason}"));
        let timing = if task.duration_ms == 0 {
            String::new()
        } else {
            let tenths = task.duration_ms / 100;
            format!("  {}.{}s", tenths / 10, tenths % 10)
        };
        lines.push(format!(
            "{:<6} {:<24}{timing}{detail}",
            status_label(task.status),
            task.name
        ));
    }
    let failed = report
        .receipt()
        .tasks
        .iter()
        .filter(|task| task.status.fails_run())
        .count();
    let passed = report
        .receipt()
        .tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Passed)
        .count();
    lines.push(String::new());
    lines.push(match report.receipt().status {
        RunStatus::Passed => format!("qualification passed: {passed} passed"),
        RunStatus::Failed => {
            format!("qualification failed: {failed} failed or blocked, {passed} passed")
        }
        RunStatus::Cancelled => format!(
            "qualification cancelled: {failed} failed, blocked, or cancelled, {passed} passed"
        ),
    });
    for task in report
        .receipt()
        .tasks
        .iter()
        .filter(|task| task.status.fails_run())
    {
        lines.push(format!("rerun: zcheck run {}", task.name));
        if let Some(log) = &task.log {
            lines.push(format!("log: {log}"));
        }
    }
    lines.push(format!("logs: {}", report.receipt().logs_dir));
    lines.push(format!("receipt: {}", report.receipt().receipt));
    Ok(lines.join("\n"))
}
