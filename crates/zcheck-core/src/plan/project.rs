//! Projection from validated tasks into value-only plan entries.

use crate::{DurationSpec, Task, TaskName};

use super::{PlannedTask, PlannedTaskKind, PlatformApplicability};

pub(super) fn task(
    name: &TaskName,
    task: &Task,
    applicability: PlatformApplicability,
    default_timeout: Option<&DurationSpec>,
) -> PlannedTask {
    let executable = task.run().is_some();
    PlannedTask {
        name: name.clone(),
        description: task.description().to_owned(),
        kind: if executable {
            PlannedTaskKind::Executable
        } else {
            PlannedTaskKind::Aggregate
        },
        applicability,
        command: task.run().map(<[String]>::to_vec),
        needs: task.needs().to_vec(),
        cwd: task.cwd().unwrap_or(".").to_owned(),
        environment: task.env().keys().cloned().collect(),
        tools: task.tools().to_vec(),
        resources: task.resources().to_vec(),
        inputs: task.inputs().to_vec(),
        timeout: if executable {
            task.timeout().or(default_timeout).cloned()
        } else {
            None
        },
    }
}
