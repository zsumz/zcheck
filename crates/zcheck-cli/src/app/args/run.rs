//! Parsing for task execution and its scheduling options.

use std::path::PathBuf;

use super::{Command, RunFormat, parse_run_format, set_once};
use crate::app::error::AppError;

pub(super) fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Command, AppError> {
    let mut tasks = Vec::new();
    let mut format = RunFormat::Human;
    let mut passthrough = Vec::new();
    let mut logs_dir = None;
    let mut receipt = None;
    let mut jobs = None;
    let mut fail_fast = false;
    let mut values = arguments.into_iter();
    while let Some(value) = values.next() {
        if value == "--" {
            passthrough.extend(values);
            break;
        }
        if value == "--format" {
            let value = values
                .next()
                .ok_or_else(|| AppError::usage("--format requires human, json, or github"))?;
            format = parse_run_format(&value)?;
        } else if value == "--logs-dir" {
            let path = values
                .next()
                .ok_or_else(|| AppError::usage("--logs-dir requires a path"))?;
            set_once(&mut logs_dir, PathBuf::from(path), "--logs-dir")?;
        } else if value == "--receipt" {
            let path = values
                .next()
                .ok_or_else(|| AppError::usage("--receipt requires a path"))?;
            set_once(&mut receipt, PathBuf::from(path), "--receipt")?;
        } else if value == "--jobs" {
            let value = values
                .next()
                .ok_or_else(|| AppError::usage("--jobs requires a positive integer"))?;
            let value = value.parse::<usize>().map_err(|_| {
                AppError::usage(format!(
                    "invalid --jobs value `{value}`: expected a positive integer"
                ))
            })?;
            if value == 0 {
                return Err(AppError::usage("--jobs must be at least 1"));
            }
            set_once(&mut jobs, value, "--jobs")?;
        } else if value == "--fail-fast" {
            if fail_fast {
                return Err(AppError::usage("--fail-fast may be provided only once"));
            }
            fail_fast = true;
        } else if value.starts_with('-') {
            return Err(AppError::usage(format!("unknown run option `{value}`")));
        } else {
            tasks.push(value);
        }
    }
    Ok(Command::Run {
        tasks,
        format,
        passthrough,
        logs_dir,
        receipt,
        jobs,
        fail_fast,
    })
}
