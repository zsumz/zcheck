//! Whole-manifest semantic and task-graph validation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;

use crate::{Error, Manifest, TaskName};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Visit {
    Visiting(usize),
    Complete,
}

pub(crate) fn manifest(manifest: &Manifest) -> Result<(), Error> {
    if manifest.schema != Manifest::SCHEMA {
        return Err(Error::new(format!(
            "unsupported manifest schema {}; this zcheck supports schema {}",
            manifest.schema,
            Manifest::SCHEMA
        )));
    }
    if manifest.execution.jobs() == 0 {
        return Err(Error::new("execution.jobs must be at least 1"));
    }
    if manifest.tasks.is_empty() {
        return Err(Error::new("manifest must define at least one task"));
    }
    if let Some(default) = &manifest.default
        && !manifest.tasks.contains_key(default)
    {
        return Err(Error::new(format!(
            "default task `{default}` is not defined"
        )));
    }
    for (name, task) in &manifest.tasks {
        task_shape(name, task)?;
        for dependency in task.needs() {
            if !manifest.tasks.contains_key(dependency) {
                return Err(Error::new(format!(
                    "task `{name}` needs unknown task `{dependency}`"
                )));
            }
        }
    }
    cycles(manifest)
}

fn task_shape(name: &TaskName, task: &crate::Task) -> Result<(), Error> {
    if task.run().is_none() && task.needs().is_empty() {
        return Err(Error::new(format!(
            "task `{name}` must define run, needs, or both"
        )));
    }
    if task.run().is_none() {
        for (field, present) in [
            ("cwd", task.cwd().is_some()),
            ("env", !task.env().is_empty()),
            ("tools", !task.tools().is_empty()),
            ("timeout", task.timeout().is_some()),
            ("resources", !task.resources().is_empty()),
        ] {
            if present {
                return Err(Error::new(format!(
                    "aggregate task `{name}` cannot define process field `{field}` without run"
                )));
            }
        }
    }
    if let Some(command) = task.run() {
        if command.is_empty() {
            return Err(Error::new(format!("task `{name}` run must not be empty")));
        }
        if command.first().is_none_or(String::is_empty) {
            return Err(Error::new(format!(
                "task `{name}` executable must not be empty"
            )));
        }
        if let Some(index) = command.iter().position(|argument| argument.contains('\0')) {
            return Err(Error::new(format!(
                "task `{name}` run argument {index} contains a NUL byte"
            )));
        }
        if let Some(executable) = command.first()
            && is_relative_program(executable)
        {
            repository_path(name, "executable", executable)?;
        }
    }
    unique(name, "dependency", task.needs())?;
    unique(name, "tool", task.tools())?;
    unique(name, "platform", task.platforms())?;
    unique(name, "resource", task.resources())?;
    unique(name, "input", task.inputs())?;
    if let Some(cwd) = task.cwd() {
        repository_path(name, "cwd", cwd)?;
    }
    for input in task.inputs() {
        repository_path(name, "input", input)?;
    }
    for tool in task.tools() {
        nonempty(name, "tool", tool)?;
        if tool.contains('\0') {
            return Err(Error::new(format!(
                "task `{name}` contains a tool with a NUL byte"
            )));
        }
        if is_relative_program(tool) {
            repository_path(name, "tool", tool)?;
        }
    }
    for resource in task.resources() {
        nonempty(name, "resource", resource)?;
    }
    for key in task.env().keys() {
        if key.is_empty() || key.contains(['=', '\0']) {
            return Err(Error::new(format!(
                "task `{name}` has invalid environment-variable name `{key}`"
            )));
        }
    }
    for value in task.env().values() {
        if value.contains('\0') {
            return Err(Error::new(format!(
                "task `{name}` has an environment value containing a NUL byte"
            )));
        }
    }
    Ok(())
}

fn nonempty(name: &TaskName, field: &str, value: &str) -> Result<(), Error> {
    if value.is_empty() {
        Err(Error::new(format!(
            "task `{name}` contains an empty {field}"
        )))
    } else {
        Ok(())
    }
}

fn unique<T>(name: &TaskName, field: &str, values: &[T]) -> Result<(), Error>
where
    T: Ord + Display,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(Error::new(format!(
                "task `{name}` contains duplicate {field} `{value}`"
            )));
        }
    }
    Ok(())
}

fn repository_path(name: &TaskName, field: &str, value: &str) -> Result<(), Error> {
    let windows_prefix = value.as_bytes().get(1).is_some_and(|byte| *byte == b':');
    let escapes = value.split(['/', '\\']).any(|component| component == "..");
    if value.is_empty()
        || value.starts_with(['/', '\\'])
        || windows_prefix
        || escapes
        || value.contains('\0')
    {
        return Err(Error::new(format!(
            "task `{name}` {field} `{value}` must stay within the repository root"
        )));
    }
    Ok(())
}

fn is_relative_program(executable: &str) -> bool {
    !is_external_absolute_program(executable)
        && (executable.contains(['/', '\\']) || executable.starts_with('.'))
}

fn is_external_absolute_program(executable: &str) -> bool {
    if executable.starts_with(['/', '\\']) {
        return true;
    }
    let bytes = executable.as_bytes();
    bytes.get(1) == Some(&b':')
        && bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes
            .get(2)
            .is_some_and(|byte| matches!(byte, b'/' | b'\\'))
}

fn cycles(manifest: &Manifest) -> Result<(), Error> {
    let mut visits = BTreeMap::new();
    let mut stack = Vec::new();
    for name in manifest.tasks.keys() {
        visit(manifest, name, &mut visits, &mut stack)?;
    }
    Ok(())
}

fn visit(
    manifest: &Manifest,
    name: &TaskName,
    visits: &mut BTreeMap<TaskName, Visit>,
    stack: &mut Vec<TaskName>,
) -> Result<(), Error> {
    match visits.get(name).copied() {
        Some(Visit::Complete) => return Ok(()),
        Some(Visit::Visiting(start)) => {
            let mut cycle = stack[start..].to_vec();
            cycle.push(name.clone());
            let path = cycle
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Error::new(format!("invalid task graph:\n{path}")));
        }
        None => {}
    }
    visits.insert(name.clone(), Visit::Visiting(stack.len()));
    stack.push(name.clone());
    if let Some(task) = manifest.tasks.get(name) {
        for dependency in task.needs() {
            visit(manifest, dependency, visits, stack)?;
        }
    }
    stack.pop();
    visits.insert(name.clone(), Visit::Complete);
    Ok(())
}

#[cfg(test)]
#[path = "validate_test.rs"]
mod validate_test;
