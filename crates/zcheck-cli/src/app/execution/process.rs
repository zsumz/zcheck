//! Direct process-group execution with streamed output capture.

use std::path::Path;
use std::time::Instant;

use zcheck_core::{DurationSpec, PlannedTask, Task, Termination};

use super::super::error::AppError;
use super::cancellation::Cancellation;
use super::logs::TaskLog;

mod capture;
mod child;
mod control;

#[derive(Debug)]
pub(super) struct ProcessResult {
    pub(super) command: Vec<String>,
    pub(super) duration_ms: u64,
    pub(super) exit_code: Option<i32>,
    pub(super) launch_error: Option<String>,
    pub(super) termination: Option<Termination>,
}

pub(super) fn arguments(
    planned: &PlannedTask,
    extra_arguments: &[String],
) -> Result<Vec<String>, AppError> {
    let mut arguments = planned.command().map_or_else(Vec::new, <[String]>::to_vec);
    arguments.extend_from_slice(extra_arguments);
    if arguments.is_empty() {
        return Err(AppError::internal(format!(
            "planned executable task `{}` has no command",
            planned.name()
        )));
    }
    Ok(arguments)
}

pub(super) fn run(
    program: &Path,
    cwd: &Path,
    task: &Task,
    arguments: Vec<String>,
    timeout: Option<&DurationSpec>,
    cancellation: &Cancellation,
    log: &TaskLog,
) -> Result<ProcessResult, AppError> {
    let started = Instant::now();
    let mut child = match child::ManagedChild::spawn(program, cwd, task, &arguments) {
        Ok(child) => child,
        Err(error) => {
            let message = error.to_string();
            log.write_launch_error(&message)?;
            return Ok(ProcessResult {
                command: arguments,
                duration_ms: elapsed_milliseconds(started),
                exit_code: None,
                launch_error: Some(message),
                termination: None,
            });
        }
    };
    let readers = match capture::Readers::start(&mut child, log) {
        Ok(readers) => readers,
        Err(error) => {
            let _ = control::force_and_wait(&mut child);
            return Err(error);
        }
    };
    let completion = match control::wait(&mut child, started, timeout, cancellation, &readers) {
        Ok(completion) => completion,
        Err(error) => {
            let _ = control::force_and_wait(&mut child);
            readers.finish()?;
            return Err(error);
        }
    };
    readers.finish()?;
    Ok(ProcessResult {
        command: arguments,
        duration_ms: elapsed_milliseconds(started),
        exit_code: completion.status.code(),
        launch_error: None,
        termination: completion.termination,
    })
}

fn elapsed_milliseconds(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
