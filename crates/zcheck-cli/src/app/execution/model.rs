//! CLI composition around the stable schema-1 receipt model.

use zcheck_core::{Receipt, RunStatus, TaskResult, Termination};

use super::super::exit;

pub(crate) use zcheck_core::TaskStatus;

pub(super) struct TaskResultData {
    pub(super) name: zcheck_core::TaskName,
    pub(super) status: TaskStatus,
    pub(super) command: Option<Vec<String>>,
    pub(super) cwd: String,
    pub(super) duration_ms: u64,
    pub(super) exit_code: Option<i32>,
    pub(super) log: Option<String>,
    pub(super) reason: Option<String>,
    pub(super) environment: Vec<String>,
    pub(super) termination: Option<Termination>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ExecutionOptions {
    pub(crate) jobs: Option<usize>,
    pub(crate) fail_fast: bool,
    pub(crate) progress: bool,
}

impl TaskResultData {
    pub(super) fn build(self) -> TaskResult {
        TaskResult {
            name: self.name,
            status: self.status,
            command: self.command,
            cwd: self.cwd,
            duration_ms: self.duration_ms,
            exit_code: self.exit_code,
            log: self.log,
            reason: self.reason,
            environment: self.environment,
            termination: self.termination,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RunReport {
    project: String,
    receipt: Receipt,
}

impl RunReport {
    pub(crate) fn new(project: String, receipt: Receipt) -> Self {
        Self { project, receipt }
    }

    pub(crate) const fn exit_code(&self) -> i32 {
        match self.receipt.status {
            RunStatus::Passed => exit::PASSED,
            RunStatus::Failed => exit::QUALIFICATION_FAILED,
            RunStatus::Cancelled => exit::INTERRUPTED,
        }
    }

    pub(crate) fn project(&self) -> &str {
        &self.project
    }

    pub(crate) const fn receipt(&self) -> &Receipt {
        &self.receipt
    }
}

pub(crate) const fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Passed => "PASS",
        TaskStatus::Failed => "FAIL",
        TaskStatus::Blocked => "BLOCK",
        TaskStatus::Skipped => "SKIP",
        TaskStatus::Cancelled => "CANCEL",
    }
}

#[cfg(test)]
#[path = "model_test.rs"]
mod model_test;
