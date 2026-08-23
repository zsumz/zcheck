//! Deterministic readiness, result, and invocation-local resource state.

use std::collections::{BTreeMap, BTreeSet};

use zcheck_core::{Plan, PlannedTaskKind, PlatformApplicability, TaskName, TaskResult, TaskStatus};

use super::super::super::error::AppError;
use super::super::reporter::Reporter;
use super::super::result::{aggregate_status, blocked_dependency, cancelled, fail_fast, synthetic};

pub(super) struct SchedulerState<'a> {
    plan: &'a Plan,
    blocked: &'a BTreeMap<TaskName, String>,
    results: Vec<Option<TaskResult>>,
    statuses: BTreeMap<TaskName, TaskStatus>,
    running: BTreeSet<usize>,
    resources: BTreeSet<String>,
    fail_fast: bool,
    first_failure: Option<TaskName>,
}

impl<'a> SchedulerState<'a> {
    pub(super) fn new(
        plan: &'a Plan,
        blocked: &'a BTreeMap<TaskName, String>,
        fail_fast: bool,
    ) -> Self {
        Self {
            plan,
            blocked,
            results: (0..plan.tasks().len()).map(|_| None).collect(),
            statuses: BTreeMap::new(),
            running: BTreeSet::new(),
            resources: BTreeSet::new(),
            fail_fast,
            first_failure: None,
        }
    }

    pub(super) fn settle(
        &mut self,
        interrupted: bool,
        reporter: &Reporter,
    ) -> Result<(), AppError> {
        loop {
            let mut changed = false;
            for index in 0..self.plan.tasks().len() {
                if !self.pending(index) {
                    continue;
                }
                let task = &self.plan.tasks()[index];
                let result = if task.applicability() == PlatformApplicability::Skipped {
                    Some(synthetic(
                        task,
                        TaskStatus::Skipped,
                        Some("platform does not apply".to_owned()),
                    ))
                } else if interrupted {
                    Some(cancelled(task))
                } else if !self.dependencies_complete(index) {
                    None
                } else if task.kind() == PlannedTaskKind::Aggregate {
                    Some(synthetic(
                        task,
                        aggregate_status(self.plan, task, &self.statuses)?,
                        None,
                    ))
                } else if let Some(reason) = blocked_dependency(task, &self.statuses)? {
                    Some(synthetic(task, TaskStatus::Blocked, Some(reason)))
                } else if let Some(reason) = self.blocked.get(task.name()) {
                    Some(synthetic(task, TaskStatus::Blocked, Some(reason.clone())))
                } else {
                    self.first_failure
                        .as_ref()
                        .filter(|_| self.fail_fast)
                        .map(|failure| fail_fast(task, failure))
                };
                if let Some(result) = result {
                    self.record(index, result, reporter)?;
                    changed = true;
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }

    pub(super) fn ready_indices(&self) -> Vec<usize> {
        (0..self.plan.tasks().len())
            .filter(|index| {
                self.pending(*index)
                    && self.plan.tasks()[*index].kind() == PlannedTaskKind::Executable
                    && self.dependencies_complete(*index)
            })
            .collect()
    }

    pub(super) fn resources_available(&self, index: usize) -> bool {
        self.plan.tasks()[index]
            .resources()
            .iter()
            .all(|resource| !self.resources.contains(resource))
    }

    pub(super) fn start(&mut self, index: usize) -> Result<(), AppError> {
        if !self.pending(index) || !self.resources_available(index) {
            return Err(AppError::internal(format!(
                "task {} was started from an invalid scheduler state",
                self.plan.tasks()[index].name()
            )));
        }
        self.running.insert(index);
        self.resources
            .extend(self.plan.tasks()[index].resources().iter().cloned());
        Ok(())
    }

    pub(super) fn finish(
        &mut self,
        index: usize,
        result: TaskResult,
        reporter: &Reporter,
    ) -> Result<(), AppError> {
        self.release(index)?;
        self.record(index, result, reporter)
    }

    pub(super) fn worker_failed(&mut self, index: usize) -> Result<(), AppError> {
        self.release(index)
    }

    pub(super) fn active(&self) -> usize {
        self.running.len()
    }

    pub(super) fn fail_fast_triggered(&self) -> bool {
        self.fail_fast && self.first_failure.is_some()
    }

    pub(super) fn complete(&self) -> bool {
        self.results.iter().all(Option::is_some)
    }

    pub(super) fn into_results(self) -> Result<Vec<TaskResult>, AppError> {
        self.results
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                result.ok_or_else(|| {
                    AppError::internal(format!("task result {index} was never resolved"))
                })
            })
            .collect()
    }

    fn pending(&self, index: usize) -> bool {
        self.results[index].is_none() && !self.running.contains(&index)
    }

    fn dependencies_complete(&self, index: usize) -> bool {
        self.plan.tasks()[index]
            .needs()
            .iter()
            .all(|dependency| self.statuses.contains_key(dependency))
    }

    fn release(&mut self, index: usize) -> Result<(), AppError> {
        if !self.running.remove(&index) {
            return Err(AppError::internal(format!(
                "task result {index} did not belong to a running worker"
            )));
        }
        for resource in self.plan.tasks()[index].resources() {
            if !self.resources.remove(resource) {
                return Err(AppError::internal(format!(
                    "task {} lost resource `{resource}`",
                    self.plan.tasks()[index].name()
                )));
            }
        }
        Ok(())
    }

    fn record(
        &mut self,
        index: usize,
        result: TaskResult,
        reporter: &Reporter,
    ) -> Result<(), AppError> {
        let task = &self.plan.tasks()[index];
        if result.name != *task.name() || self.results[index].is_some() {
            return Err(AppError::internal(format!(
                "task result `{}` does not match scheduler slot {}",
                result.name,
                task.name()
            )));
        }
        if self.fail_fast
            && self.first_failure.is_none()
            && matches!(result.status, TaskStatus::Failed | TaskStatus::Blocked)
        {
            self.first_failure = Some(result.name.clone());
        }
        self.statuses.insert(result.name.clone(), result.status);
        reporter.finished(&result);
        self.results[index] = Some(result);
        Ok(())
    }
}
