//! Deterministic GitHub Actions groups and bounded failure annotations.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use zcheck_core::{TaskResult, TaskStatus};

use super::model::{RunReport, status_label};

const MAX_ANNOTATIONS: usize = 10;
const MAX_ANNOTATION_BYTES: usize = 4 * 1024;
const MAX_ANNOTATION_TITLE_BYTES: usize = 256;
const MAX_CONTEXT_BYTES: usize = 16 * 1024;
const MAX_CONTEXT_LINES: usize = 50;

pub(super) fn render(report: &RunReport) -> String {
    let mut lines = Vec::new();
    let failures = report
        .receipt()
        .tasks
        .iter()
        .filter(|task| task.status.fails_run())
        .collect::<Vec<_>>();
    let visible = if failures.len() > MAX_ANNOTATIONS {
        MAX_ANNOTATIONS.saturating_sub(1)
    } else {
        failures.len()
    };
    for task in failures.iter().take(visible) {
        lines.push(annotation(task));
    }
    if failures.len() > visible {
        let hidden = failures.len() - visible;
        lines.push(command(
            "error",
            Some("zcheck additional failures"),
            &format!(
                "{hidden} additional failed, blocked, or cancelled task(s); receipt: {}",
                report.receipt().receipt
            ),
        ));
    }
    for task in &report.receipt().tasks {
        group(task, &mut lines);
    }
    summary(report, &mut lines);
    lines.join("\n")
}

fn annotation(task: &TaskResult) -> String {
    let reason = task.reason.as_deref().unwrap_or("task did not pass");
    let mut message = format!("{reason}\nrerun: zcheck run {}", task.name);
    if let Some(path) = &task.log {
        message.push_str("\nlog: ");
        message.push_str(path);
        if let Ok(context) = read_tail(Path::new(path))
            && !context.is_empty()
        {
            message.push_str("\n\nlast log lines:\n");
            message.push_str(&context);
        }
    }
    command(
        "error",
        Some(&format!(
            "zcheck {}: {}",
            status_name(task.status),
            task.name
        )),
        &message,
    )
}

fn group(task: &TaskResult, lines: &mut Vec<String>) {
    lines.push(format!(
        "::group::{}",
        escape_data(&format!(
            "{} {} ({})",
            status_label(task.status),
            task.name,
            duration(task.duration_ms)
        ))
    ));
    lines.push(format!("| status: {}", status_name(task.status)));
    if let Some(reason) = &task.reason {
        safe_lines("reason", reason, lines);
    }
    if let Some(command) = &task.command {
        lines.push(format!("| command: {command:?}"));
    }
    safe_lines("cwd", &task.cwd, lines);
    if let Some(path) = &task.log {
        safe_lines("log", path, lines);
        if task.status.fails_run()
            && let Ok(context) = read_tail(Path::new(path))
            && !context.is_empty()
        {
            lines.push("| last log lines:".to_owned());
            safe_lines("", &context, lines);
        }
    }
    lines.push("::endgroup::".to_owned());
}

fn summary(report: &RunReport, lines: &mut Vec<String>) {
    lines.push("::group::zcheck summary".to_owned());
    lines.push(format!("| status: {}", run_status(report)));
    safe_lines("logs", &report.receipt().logs_dir, lines);
    safe_lines("receipt", &report.receipt().receipt, lines);
    lines.push("::endgroup::".to_owned());
}

fn command(name: &str, title: Option<&str>, message: &str) -> String {
    let property = title.map_or_else(String::new, |value| {
        format!(
            " title={}",
            bounded(&escape_property(value), MAX_ANNOTATION_TITLE_BYTES)
        )
    });
    let message = bounded(&escape_data(message), MAX_ANNOTATION_BYTES);
    format!("::{name}{property}::{message}")
}

fn safe_lines(label: &str, value: &str, lines: &mut Vec<String>) {
    for (index, line) in value.lines().enumerate() {
        let prefix = if index == 0 && !label.is_empty() {
            format!("{label}: ")
        } else {
            String::new()
        };
        lines.push(format!("| {prefix}{}", line.replace(['\0', '\r'], "�")));
    }
}

fn read_tail(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(MAX_CONTEXT_BYTES as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity(MAX_CONTEXT_BYTES);
    file.take(MAX_CONTEXT_BYTES as u64)
        .read_to_end(&mut bytes)?;
    if start > 0 {
        if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=newline);
        } else {
            bytes.clear();
        }
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text
        .lines()
        .rev()
        .take(MAX_CONTEXT_LINES)
        .collect::<Vec<_>>();
    lines.reverse();
    Ok(lines.join("\n"))
}

fn bounded(value: &str, limit: usize) -> String {
    const SUFFIX: &str = " [truncated by zcheck]";
    if value.len() <= limit {
        return value.to_owned();
    }
    let mut end = limit.saturating_sub(SUFFIX.len()).min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}{SUFFIX}", &value[..end])
}

fn escape_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(value: &str) -> String {
    escape_data(value).replace(':', "%3A").replace(',', "%2C")
}

fn duration(milliseconds: u64) -> String {
    let tenths = milliseconds / 100;
    format!("{}.{}s", tenths / 10, tenths % 10)
}

const fn status_name(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Passed => "passed",
        TaskStatus::Failed => "failed",
        TaskStatus::Blocked => "blocked",
        TaskStatus::Skipped => "skipped",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn run_status(report: &RunReport) -> &'static str {
    match report.receipt().status {
        zcheck_core::RunStatus::Passed => "passed",
        zcheck_core::RunStatus::Failed => "failed",
        zcheck_core::RunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
#[path = "github_test.rs"]
mod github_test;
