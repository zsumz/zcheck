//! Scheduler result derivation for synthetic and aggregate tasks.

use std::collections::BTreeMap;

use zcheck_core::{
    Plan, PlannedTask, PlatformApplicability, TaskName, TaskResult, TerminationReason,
};

use super::super::error::AppError;
use super::model::{TaskResultData, TaskStatus};

pub(super) fn termination_result(
    task: &PlannedTask,
    reason: TerminationReason,
    forced: bool,
) -> (TaskStatus, Option<String>) {
    let escalation = if forced {
        "; forced process-tree termination was required"
    } else {
        ""
    };
    match reason {
        TerminationReason::Timeout => {
            let timeout = task
                .timeout()
                .map_or("configured limit", |value| value.as_str());
            (
                TaskStatus::Failed,
                Some(format!("timed out after {timeout}{escalation}")),
            )
        }
        TerminationReason::Interrupted => (
            TaskStatus::Cancelled,
            Some(format!("interrupted by user{escalation}")),
        ),
    }
}

pub(super) fn cancelled(task: &PlannedTask) -> TaskResult {
    if task.applicability() == PlatformApplicability::Skipped {
        synthetic(
            task,
            TaskStatus::Skipped,
            Some("platform does not apply".to_owned()),
        )
    } else {
        synthetic(
            task,
            TaskStatus::Cancelled,
            Some("invocation interrupted before task started".to_owned()),
        )
    }
}

pub(super) fn fail_fast(task: &PlannedTask, failure: &TaskName) -> TaskResult {
    synthetic(
        task,
        TaskStatus::Blocked,
        Some(format!(
            "fail-fast stopped scheduling after task `{failure}` did not pass"
        )),
    )
}

pub(super) fn blocked_dependency(
    task: &PlannedTask,
    statuses: &BTreeMap<TaskName, TaskStatus>,
) -> Result<Option<String>, AppError> {
    for dependency in task.needs() {
        let status = statuses.get(dependency).ok_or_else(|| {
            AppError::internal(format!(
                "task `{}` was scheduled before dependency `{dependency}`",
                task.name()
            ))
        })?;
        if status.fails_run() {
            return Ok(Some(
                format!("dependency `{dependency}` {status:?}").to_lowercase(),
            ));
        }
    }
    Ok(None)
}

pub(super) fn aggregate_status(
    plan: &Plan,
    task: &PlannedTask,
    statuses: &BTreeMap<TaskName, TaskStatus>,
) -> Result<TaskStatus, AppError> {
    let mut pending = task.needs().to_vec();
    let mut dependencies = Vec::new();
    while let Some(name) = pending.pop() {
        let status = statuses.get(&name).copied().ok_or_else(|| {
            AppError::internal(format!(
                "aggregate {} is missing dependency {name}",
                task.name()
            ))
        })?;
        dependencies.push(status);
        let planned = plan
            .tasks()
            .iter()
            .find(|candidate| candidate.name() == &name)
            .ok_or_else(|| AppError::internal(format!("plan is missing task {name}")))?;
        if status != TaskStatus::Skipped {
            pending.extend_from_slice(planned.needs());
        }
    }
    Ok(if dependencies.contains(&TaskStatus::Failed) {
        TaskStatus::Failed
    } else if dependencies.contains(&TaskStatus::Blocked) {
        TaskStatus::Blocked
    } else if dependencies.contains(&TaskStatus::Cancelled) {
        TaskStatus::Cancelled
    } else if dependencies
        .iter()
        .all(|status| *status == TaskStatus::Skipped)
    {
        TaskStatus::Skipped
    } else {
        TaskStatus::Passed
    })
}

pub(super) fn synthetic(
    task: &PlannedTask,
    status: TaskStatus,
    reason: Option<String>,
) -> TaskResult {
    TaskResultData {
        name: task.name().clone(),
        status,
        command: task.command().map(<[String]>::to_vec),
        cwd: task.cwd().to_owned(),
        duration_ms: 0,
        exit_code: None,
        log: None,
        reason,
        environment: Vec::new(),
        termination: None,
    }
    .build()
}
