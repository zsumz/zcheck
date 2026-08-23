//! Execution of one preflighted task inside a scheduler worker.

use zcheck_core::{Plan, PlannedTask, TaskResult};

use super::super::discovery::Repository;
use super::super::error::AppError;
use super::cancellation::Cancellation;
use super::logs::RunLogs;
use super::model::{TaskResultData, TaskStatus};
use super::preflight;
use super::process;
use super::result::termination_result;

pub(super) struct TaskExecution<'a> {
    pub(super) repository: &'a Repository,
    pub(super) plan: &'a Plan,
    pub(super) passthrough: &'a [String],
    pub(super) preflight: &'a preflight::Preflight,
    pub(super) logs: &'a RunLogs,
    pub(super) cancellation: &'a Cancellation,
}

pub(super) fn execute(
    task: &PlannedTask,
    execution: &TaskExecution<'_>,
) -> Result<TaskResult, AppError> {
    let declared = execution
        .repository
        .manifest
        .task(task.name())
        .ok_or_else(|| {
            AppError::internal(format!(
                "planned task `{}` disappeared from manifest",
                task.name()
            ))
        })?;
    let extra =
        if execution.plan.selection().len() == 1 && execution.plan.selection()[0] == *task.name() {
            execution.passthrough
        } else {
            &[]
        };
    let paths = execution.preflight.task(task.name()).ok_or_else(|| {
        AppError::internal(format!("task `{}` has no preflight result", task.name()))
    })?;
    let program = paths.program().ok_or_else(|| {
        AppError::internal(format!(
            "task `{}` passed preflight without a program",
            task.name()
        ))
    })?;
    let cwd = paths.cwd().ok_or_else(|| {
        AppError::internal(format!(
            "task `{}` passed preflight without a cwd",
            task.name()
        ))
    })?;
    let arguments = process::arguments(task, extra)?;
    let task_log =
        execution
            .logs
            .start(task.name(), cwd, declared.env().keys().cloned(), &arguments)?;
    let outcome = process::run(
        program,
        cwd,
        declared,
        arguments,
        task.timeout(),
        execution.cancellation,
        &task_log,
    )?;
    let log = task_log.finish()?;
    let (status, reason) = if let Some(termination) = &outcome.termination {
        termination_result(task, termination.reason, termination.forced)
    } else if let Some(error) = &outcome.launch_error {
        (
            TaskStatus::Blocked,
            Some(format!("could not start process: {error}")),
        )
    } else if outcome.exit_code == Some(0) {
        (TaskStatus::Passed, None)
    } else {
        let reason = outcome.exit_code.map_or_else(
            || "process terminated without an exit code".to_owned(),
            |code| format!("process exited with code {code}"),
        );
        (TaskStatus::Failed, Some(reason))
    };
    Ok(TaskResultData {
        name: task.name().clone(),
        status,
        command: Some(outcome.command),
        cwd: task.cwd().to_owned(),
        duration_ms: outcome.duration_ms,
        exit_code: outcome.exit_code,
        log: Some(log.display().to_string()),
        reason,
        environment: declared.env().keys().cloned().collect(),
        termination: outcome.termination,
    }
    .build())
}
