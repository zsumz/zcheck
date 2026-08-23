//! Validated schema-1 manifest data model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{DurationSpec, Platform, TaskName};

/// A validated schema-1 zcheck manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub(crate) schema: u32,
    pub(crate) default: Option<TaskName>,
    pub(crate) execution: ExecutionPolicy,
    pub(crate) tasks: BTreeMap<TaskName, Task>,
}

impl Manifest {
    /// Manifest schema version accepted by this release line.
    pub const SCHEMA: u32 = 1;

    /// Returns the manifest schema version.
    #[must_use]
    pub const fn schema(&self) -> u32 {
        self.schema
    }

    /// Returns the task selected by bare `zcheck` and `zcheck run`.
    #[must_use]
    pub fn default_task(&self) -> Option<&TaskName> {
        self.default.as_ref()
    }

    /// Returns the repository execution policy.
    #[must_use]
    pub const fn execution(&self) -> &ExecutionPolicy {
        &self.execution
    }

    /// Returns every task in name order.
    #[must_use]
    pub const fn tasks(&self) -> &BTreeMap<TaskName, Task> {
        &self.tasks
    }

    /// Returns one named task.
    #[must_use]
    pub fn task(&self, name: &TaskName) -> Option<&Task> {
        self.tasks.get(name)
    }
}

/// Repository-wide execution defaults.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExecutionPolicy {
    jobs: usize,
    default_timeout: Option<DurationSpec>,
    repository_state: RepositoryStatePolicy,
}

impl ExecutionPolicy {
    /// Returns the maximum concurrently running leaf-task count.
    #[must_use]
    pub const fn jobs(&self) -> usize {
        self.jobs
    }

    /// Returns the default executable-task timeout, when configured.
    #[must_use]
    pub const fn default_timeout(&self) -> Option<&DurationSpec> {
        self.default_timeout.as_ref()
    }

    /// Returns the checkout-state enforcement policy.
    #[must_use]
    pub const fn repository_state(&self) -> RepositoryStatePolicy {
        self.repository_state
    }
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            jobs: 1,
            default_timeout: None,
            repository_state: RepositoryStatePolicy::Preserve,
        }
    }
}

/// Checkout-state enforcement selected for a run.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepositoryStatePolicy {
    /// Allow initial dirt but require exact content preservation.
    #[default]
    Preserve,
    /// Require a clean checkout before and after qualification.
    Clean,
    /// Do not inspect repository state.
    Ignore,
}

/// One validated task declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Task {
    #[serde(default)]
    description: String,
    run: Option<Vec<String>>,
    #[serde(default)]
    needs: Vec<TaskName>,
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    tools: Vec<String>,
    timeout: Option<DurationSpec>,
    #[serde(default)]
    platforms: Vec<Platform>,
    #[serde(default)]
    resources: Vec<String>,
    #[serde(default)]
    inputs: Vec<String>,
}

impl Task {
    /// Returns the human-facing purpose.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the process argument vector, if this task executes.
    #[must_use]
    pub fn run(&self) -> Option<&[String]> {
        self.run.as_deref()
    }

    /// Returns hard dependencies in declared order.
    #[must_use]
    pub fn needs(&self) -> &[TaskName] {
        &self.needs
    }

    /// Returns the repository-relative working directory.
    #[must_use]
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// Returns explicit environment overrides.
    #[must_use]
    pub const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    /// Returns executables that must exist before scheduling.
    #[must_use]
    pub fn tools(&self) -> &[String] {
        &self.tools
    }

    /// Returns the task-specific timeout override.
    #[must_use]
    pub const fn timeout(&self) -> Option<&DurationSpec> {
        self.timeout.as_ref()
    }

    /// Returns the explicitly applicable platforms; empty means all.
    #[must_use]
    pub fn platforms(&self) -> &[Platform] {
        &self.platforms
    }

    /// Returns named invocation-local exclusive resources.
    #[must_use]
    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    /// Returns additional repository files governing the task.
    #[must_use]
    pub fn inputs(&self) -> &[String] {
        &self.inputs
    }

    pub(crate) fn applies_to(&self, platform: Platform) -> bool {
        self.platforms.is_empty() || self.platforms.contains(&platform)
    }
}
