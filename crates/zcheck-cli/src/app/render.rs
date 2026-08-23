//! Human and versioned JSON rendering for inspect-only commands.

use serde::Serialize;
use zcheck_core::{Manifest, Plan, PlannedTaskKind, PlatformApplicability};

use super::args::Format;
use super::error::AppError;

#[derive(Serialize)]
struct TaskListing<'a> {
    name: &'a str,
    description: &'a str,
    kind: &'static str,
}

#[derive(Serialize)]
struct ListReport<'a> {
    schema: u32,
    tasks: Vec<TaskListing<'a>>,
}

#[derive(Serialize)]
struct ValidationReport<'a> {
    schema: u32,
    manifest: &'a str,
    status: &'static str,
    tasks: usize,
}

pub(crate) fn list(manifest: &Manifest, format: Format) -> Result<String, AppError> {
    let tasks = manifest
        .tasks()
        .iter()
        .map(|(name, task)| TaskListing {
            name: name.as_str(),
            description: task.description(),
            kind: if task.run().is_some() {
                "executable"
            } else {
                "aggregate"
            },
        })
        .collect::<Vec<_>>();
    match format {
        Format::Human => Ok(tasks
            .iter()
            .map(|task| format!("{:<24} {}", task.name, task.description))
            .collect::<Vec<_>>()
            .join("\n")),
        Format::Json => json(&ListReport { schema: 1, tasks }),
    }
}

pub(crate) fn plan(plan: &Plan, format: Format) -> Result<String, AppError> {
    if format == Format::Json {
        return plan
            .to_json_pretty()
            .map_err(|error| AppError::internal(error.to_string()));
    }
    let mut lines = vec![format!(
        "plan · {} · jobs {}",
        plan.platform().as_str(),
        plan.jobs()
    )];
    for task in plan.tasks() {
        let status = if task.applicability() == PlatformApplicability::Skipped {
            "SKIP"
        } else if task.kind() == PlannedTaskKind::Aggregate {
            "AGGR"
        } else {
            "RUN "
        };
        let detail = if let Some(command) = task.command() {
            command
                .iter()
                .map(|argument| {
                    serde_json::to_string(argument)
                        .map_err(|error| AppError::internal(format!("cannot quote argv: {error}")))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(" ")
        } else {
            task.needs()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        lines.push(format!("{status} {:<24} {detail}", task.name()));
    }
    Ok(lines.join("\n"))
}

pub(crate) fn validation(
    manifest: &Manifest,
    path: &str,
    format: Format,
) -> Result<String, AppError> {
    if format == Format::Human {
        return Ok(format!(
            "{path}: valid (schema {}, {} tasks)",
            manifest.schema(),
            manifest.tasks().len()
        ));
    }
    json(&ValidationReport {
        schema: 1,
        manifest: path,
        status: "valid",
        tasks: manifest.tasks().len(),
    })
}

fn json(value: &impl Serialize) -> Result<String, AppError> {
    serde_json::to_string_pretty(value)
        .map_err(|error| AppError::internal(format!("cannot render JSON: {error}")))
}
