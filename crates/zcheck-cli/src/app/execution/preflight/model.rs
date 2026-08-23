//! Resolved task launch facts produced by preflight.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use zcheck_core::TaskName;

pub(super) enum CwdResolution {
    Ready(PathBuf),
    Blocked(String),
}

#[derive(Debug)]
pub(crate) struct Preflight {
    tasks: BTreeMap<TaskName, TaskPreflight>,
}

impl Preflight {
    pub(super) const fn new(tasks: BTreeMap<TaskName, TaskPreflight>) -> Self {
        Self { tasks }
    }

    pub(crate) fn task(&self, name: &TaskName) -> Option<&TaskPreflight> {
        self.tasks.get(name)
    }

    pub(crate) fn blocked_tasks(&self) -> BTreeMap<TaskName, String> {
        self.tasks
            .iter()
            .filter_map(|(name, task)| {
                task.blocked
                    .as_ref()
                    .map(|reason| (name.clone(), reason.clone()))
            })
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct TaskPreflight {
    pub(super) cwd: Option<PathBuf>,
    pub(super) program: Option<PathBuf>,
    pub(super) blocked: Option<String>,
}

impl TaskPreflight {
    pub(crate) fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    pub(crate) fn program(&self) -> Option<&Path> {
        self.program.as_deref()
    }
}
