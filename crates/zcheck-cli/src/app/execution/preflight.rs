//! Executable discovery and repository-local path containment.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use zcheck_core::{Manifest, Plan, PlatformApplicability, TaskName};

use super::super::error::AppError;

pub(crate) use model::Preflight;
use model::{CwdResolution, TaskPreflight};

mod model;

pub(crate) fn analyze(
    root: &Path,
    manifest: &Manifest,
    plan: &Plan,
) -> Result<Preflight, AppError> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        AppError::configuration(format!(
            "cannot resolve repository root {}: {error}",
            root.display()
        ))
    })?;
    let mut tasks = BTreeMap::new();
    for task in plan.tasks().iter().filter(|task| {
        task.applicability() == PlatformApplicability::Applicable && task.command().is_some()
    }) {
        let cwd = match task_cwd(&canonical_root, task.name(), task.cwd())? {
            CwdResolution::Ready(path) => path,
            CwdResolution::Blocked(reason) => {
                tasks.insert(
                    task.name().clone(),
                    TaskPreflight {
                        cwd: None,
                        program: None,
                        blocked: Some(reason),
                    },
                );
                continue;
            }
        };
        let declared = manifest.task(task.name()).ok_or_else(|| {
            AppError::internal(format!(
                "planned task {} is absent from its manifest",
                task.name()
            ))
        })?;
        let path = declared.env().get("PATH").map(OsStr::new);
        let command = task
            .command()
            .and_then(|arguments| arguments.first())
            .ok_or_else(|| AppError::internal(format!("task {} has no executable", task.name())))?;
        let program = resolve_tool(&canonical_root, &cwd, task.name(), command, path)?;
        let mut missing = BTreeSet::new();
        if program.is_none() {
            missing.insert(command.clone());
        }
        for tool in task.tools() {
            if resolve_tool(&canonical_root, &cwd, task.name(), tool, path)?.is_none() {
                missing.insert(tool.clone());
            }
        }
        tasks.insert(
            task.name().clone(),
            TaskPreflight {
                cwd: Some(cwd),
                program,
                blocked: (!missing.is_empty()).then(|| {
                    format!(
                        "missing required tool(s): {}",
                        missing.into_iter().collect::<Vec<_>>().join(", ")
                    )
                }),
            },
        );
    }
    Ok(Preflight::new(tasks))
}

pub(crate) fn blocked_tasks(
    root: &Path,
    manifest: &Manifest,
    plan: &Plan,
) -> Result<BTreeMap<TaskName, String>, AppError> {
    Ok(analyze(root, manifest, plan)?.blocked_tasks())
}

fn task_cwd(root: &Path, name: &TaskName, declared: &str) -> Result<CwdResolution, AppError> {
    let candidate = root.join(declared);
    let resolved = match fs::canonicalize(&candidate) {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(CwdResolution::Blocked(format!(
                "working directory `{declared}` does not exist"
            )));
        }
        Err(error) => {
            return Err(AppError::configuration(format!(
                "task `{name}` cwd `{declared}` cannot be resolved: {error}"
            )));
        }
    };
    if !resolved.starts_with(root) {
        return Err(AppError::configuration(format!(
            "task `{name}` cwd `{declared}` resolves outside the repository root"
        )));
    }
    if !resolved.is_dir() {
        return Err(AppError::configuration(format!(
            "task `{name}` cwd `{declared}` is not a directory"
        )));
    }
    Ok(CwdResolution::Ready(resolved))
}

fn resolve_tool(
    root: &Path,
    cwd: &Path,
    task: &TaskName,
    tool: &str,
    overridden_path: Option<&OsStr>,
) -> Result<Option<PathBuf>, AppError> {
    let path = Path::new(tool);
    if path.is_absolute() {
        return Ok(executable(path));
    }
    if path.components().count() > 1 {
        let candidate = cwd.join(path);
        return match fs::canonicalize(&candidate) {
            Ok(resolved) if !resolved.starts_with(root) => Err(AppError::configuration(format!(
                "task `{task}` repository-local executable `{tool}` resolves outside the repository root"
            ))),
            Ok(resolved) => Ok(executable(&resolved)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(AppError::configuration(format!(
                "task `{task}` repository-local executable `{tool}` cannot be resolved: {error}"
            ))),
        };
    }
    resolve_task_on_path(root, cwd, task, path, overridden_path)
}

pub(crate) fn find_tool(cwd: &Path, tool: &str) -> Option<PathBuf> {
    let path = Path::new(tool);
    if path.is_absolute() || path.components().count() > 1 {
        return executable(&if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        });
    }
    find_on_path(cwd, path, None)
}

fn find_on_path(cwd: &Path, tool: &Path, overridden_path: Option<&OsStr>) -> Option<PathBuf> {
    overridden_path
        .map(ToOwned::to_owned)
        .or_else(|| env::var_os("PATH"))
        .into_iter()
        .flat_map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .map(|directory| {
            if directory.is_absolute() {
                directory.join(tool)
            } else {
                cwd.join(directory).join(tool)
            }
        })
        .find_map(|candidate| executable(&candidate))
}

fn resolve_task_on_path(
    root: &Path,
    cwd: &Path,
    task: &TaskName,
    tool: &Path,
    overridden_path: Option<&OsStr>,
) -> Result<Option<PathBuf>, AppError> {
    let path = overridden_path
        .map(ToOwned::to_owned)
        .or_else(|| env::var_os("PATH"));
    for directory in path
        .into_iter()
        .flat_map(|value| env::split_paths(&value).collect::<Vec<_>>())
    {
        let candidate = if directory.is_absolute() {
            directory.join(tool)
        } else {
            cwd.join(&directory).join(tool)
        };
        let Some(program) = executable(&candidate) else {
            continue;
        };
        if directory.is_absolute() {
            return Ok(Some(program));
        }
        let resolved = fs::canonicalize(&program).map_err(|error| {
            AppError::configuration(format!(
                "task `{task}` executable `{}` found through relative PATH entry `{}` cannot be resolved: {error}",
                tool.display(),
                directory.display()
            ))
        })?;
        if !resolved.starts_with(root) {
            return Err(AppError::configuration(format!(
                "task `{task}` executable `{}` found through relative PATH entry `{}` resolves outside the repository root",
                tool.display(),
                directory.display()
            )));
        }
        return Ok(Some(resolved));
    }
    Ok(None)
}

#[cfg(unix)]
fn executable(path: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = path.metadata().ok()?;
    (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then(|| path.to_path_buf())
}

#[cfg(windows)]
fn executable(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let extensions = env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    extensions
        .to_string_lossy()
        .split(';')
        .map(|extension| path.with_extension(extension.trim_start_matches('.')))
        .find(|candidate| candidate.is_file())
}
