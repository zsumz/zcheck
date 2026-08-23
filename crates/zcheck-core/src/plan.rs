//! Deterministic, platform-projected task-plan resolution.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::{DurationSpec, Error, Manifest, Platform, TaskName};

mod project;

/// A versioned deterministic plan for one selection and platform.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Plan {
    schema: u32,
    selection: Vec<TaskName>,
    platform: Platform,
    jobs: usize,
    tasks: Vec<PlannedTask>,
}

impl Plan {
    /// Plan JSON schema version emitted by this release line.
    pub const SCHEMA: u32 = 1;

    /// Returns the plan JSON schema version.
    #[must_use]
    pub const fn schema(&self) -> u32 {
        self.schema
    }

    /// Returns selected root tasks in command-line order.
    #[must_use]
    pub fn selection(&self) -> &[TaskName] {
        &self.selection
    }

    /// Returns the projected host platform.
    #[must_use]
    pub const fn platform(&self) -> Platform {
        self.platform
    }

    /// Returns the configured maximum leaf-task concurrency.
    #[must_use]
    pub const fn jobs(&self) -> usize {
        self.jobs
    }

    /// Returns reachable tasks in deterministic dependency order.
    #[must_use]
    pub fn tasks(&self) -> &[PlannedTask] {
        &self.tasks
    }

    /// Serializes this plan using the stable pretty-printed JSON contract.
    pub fn to_json_pretty(&self) -> Result<String, Error> {
        serde_json::to_string_pretty(self)
            .map_err(|error| Error::new(format!("cannot serialize plan: {error}")))
    }
}

/// One task projected into a deterministic plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedTask {
    name: TaskName,
    description: String,
    kind: PlannedTaskKind,
    applicability: PlatformApplicability,
    command: Option<Vec<String>>,
    needs: Vec<TaskName>,
    cwd: String,
    environment: Vec<String>,
    tools: Vec<String>,
    resources: Vec<String>,
    inputs: Vec<String>,
    timeout: Option<DurationSpec>,
}

impl PlannedTask {
    /// Returns the task identity.
    #[must_use]
    pub const fn name(&self) -> &TaskName {
        &self.name
    }

    /// Returns the human-facing purpose.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns whether the task executes or only aggregates dependencies.
    #[must_use]
    pub const fn kind(&self) -> PlannedTaskKind {
        self.kind
    }

    /// Returns host-platform applicability.
    #[must_use]
    pub const fn applicability(&self) -> PlatformApplicability {
        self.applicability
    }

    /// Returns the direct process argument vector, if present.
    #[must_use]
    pub fn command(&self) -> Option<&[String]> {
        self.command.as_deref()
    }

    /// Returns hard task dependencies in declaration order.
    #[must_use]
    pub fn needs(&self) -> &[TaskName] {
        &self.needs
    }

    /// Returns the effective repository-relative working directory.
    #[must_use]
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Returns explicitly overridden environment-variable names.
    #[must_use]
    pub fn environment(&self) -> &[String] {
        &self.environment
    }

    /// Returns required executable names.
    #[must_use]
    pub fn tools(&self) -> &[String] {
        &self.tools
    }

    /// Returns exclusive invocation-local resources.
    #[must_use]
    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    /// Returns additional repository-relative task inputs.
    #[must_use]
    pub fn inputs(&self) -> &[String] {
        &self.inputs
    }

    /// Returns the effective timeout.
    #[must_use]
    pub const fn timeout(&self) -> Option<&DurationSpec> {
        self.timeout.as_ref()
    }
}

/// Whether a planned task owns a process.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlannedTaskKind {
    /// A task with a direct argument vector, with or without dependencies.
    Executable,
    /// A dependency-only task.
    Aggregate,
}

/// Whether a task applies to the projected host platform.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformApplicability {
    /// The task participates in execution on this platform.
    Applicable,
    /// The task is visible but excluded on this platform.
    Skipped,
}

impl Manifest {
    /// Resolves selected roots once in deterministic dependency order.
    pub fn plan(&self, selection: &[TaskName], platform: Platform) -> Result<Plan, Error> {
        if selection.is_empty() {
            return Err(Error::new("plan selection must contain at least one task"));
        }
        for name in selection {
            if !self.tasks.contains_key(name) {
                return Err(Error::new(format!("selected task `{name}` is not defined")));
            }
        }
        let mut seen = BTreeSet::new();
        let mut tasks = Vec::new();
        for name in selection {
            self.plan_task(name, platform, &mut seen, &mut tasks);
        }
        Ok(Plan {
            schema: Plan::SCHEMA,
            selection: selection.to_vec(),
            platform,
            jobs: self.execution.jobs(),
            tasks,
        })
    }

    fn plan_task(
        &self,
        name: &TaskName,
        platform: Platform,
        seen: &mut BTreeSet<TaskName>,
        planned: &mut Vec<PlannedTask>,
    ) {
        if !seen.insert(name.clone()) {
            return;
        }
        let Some(task) = self.tasks.get(name) else {
            return;
        };
        let applicability = if task.applies_to(platform) {
            for dependency in task.needs() {
                self.plan_task(dependency, platform, seen, planned);
            }
            PlatformApplicability::Applicable
        } else {
            PlatformApplicability::Skipped
        };
        planned.push(project::task(
            name,
            task,
            applicability,
            self.execution.default_timeout(),
        ));
    }
}

#[cfg(test)]
#[path = "plan_test.rs"]
mod plan_test;
