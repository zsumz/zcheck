//! Scoped worker orchestration around deterministic scheduler state.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::thread;

use zcheck_core::{Plan, TaskResult};

use super::super::super::discovery::Repository;
use super::super::super::error::AppError;
use super::super::cancellation::Cancellation;
use super::super::logs::RunLogs;
use super::super::model::ExecutionOptions;
use super::super::preflight;
use super::super::reporter::Reporter;
use super::super::task::{self, TaskExecution};
use super::state::SchedulerState;

struct Completion {
    index: usize,
    result: Result<TaskResult, AppError>,
}

pub(super) fn execute_plan(
    repository: &Repository,
    plan: &Plan,
    passthrough: &[String],
    logs: &RunLogs,
    cancellation: &Cancellation,
    options: ExecutionOptions,
) -> Result<Vec<TaskResult>, AppError> {
    let preflight = preflight::analyze(&repository.root, &repository.manifest, plan)?;
    let blocked = preflight.blocked_tasks();
    let jobs = options.jobs.unwrap_or_else(|| plan.jobs());
    let reporter = Reporter::new(options.progress);
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        let mut state = SchedulerState::new(plan, &blocked, options.fail_fast);
        let mut internal_error = None;
        loop {
            state.settle(cancellation.requested(), &reporter)?;
            if state.complete() {
                return internal_error.map_or_else(|| state.into_results(), Err);
            }
            if internal_error.is_none() && !cancellation.requested() && !state.fail_fast_triggered()
            {
                for index in state.ready_indices() {
                    if state.active() >= jobs {
                        break;
                    }
                    if !state.resources_available(index) {
                        continue;
                    }
                    let task = plan.tasks().get(index).ok_or_else(|| {
                        AppError::internal(format!("scheduler lost task index {index}"))
                    })?;
                    let sender = sender.clone();
                    let task_name = task.name().clone();
                    let execution = TaskExecution {
                        repository,
                        plan,
                        passthrough,
                        preflight: &preflight,
                        logs,
                        cancellation,
                    };
                    let worker = thread::Builder::new()
                        .name(format!("zcheck-task-{index}"))
                        .spawn_scoped(scope, move || {
                            let result =
                                catch_unwind(AssertUnwindSafe(|| task::execute(task, &execution)))
                                    .unwrap_or_else(|_| {
                                        Err(AppError::internal(format!(
                                            "task worker `{task_name}` panicked"
                                        )))
                                    });
                            let _ = sender.send(Completion { index, result });
                        });
                    match worker {
                        Ok(_) => {
                            state.start(index)?;
                            reporter.started(task.name());
                        }
                        Err(error) => {
                            internal_error = Some(AppError::internal(format!(
                                "cannot start task worker `{}`: {error}",
                                task.name()
                            )));
                            cancellation.request();
                            break;
                        }
                    }
                }
            }
            if state.active() == 0 {
                if let Some(error) = internal_error {
                    return Err(error);
                }
                return Err(AppError::internal(
                    "task scheduler reached a pending graph with no runnable work",
                ));
            }
            let mut completions =
                vec![receiver.recv().map_err(|_| {
                    AppError::internal("task worker channel closed before completion")
                })?];
            completions.extend(receiver.try_iter());
            completions.sort_by_key(|completion| completion.index);
            for completion in completions {
                match completion.result {
                    Ok(result) => state.finish(completion.index, result, &reporter)?,
                    Err(error) => {
                        state.worker_failed(completion.index)?;
                        if internal_error.is_none() {
                            internal_error = Some(error);
                        }
                        cancellation.request();
                    }
                }
            }
        }
    })
}
