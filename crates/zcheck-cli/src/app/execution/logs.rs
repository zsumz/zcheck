//! Per-run log storage outside the repository checkout.

use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use zcheck_core::{Plan, TaskName};

use super::super::error::AppError;

mod location;

#[derive(Debug)]
pub(super) struct RunLogs {
    run_id: String,
    directory: PathBuf,
    tasks: BTreeMap<TaskName, PathBuf>,
}

impl RunLogs {
    pub(super) fn create(plan: &Plan, requested: Option<&Path>) -> Result<Self, AppError> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                AppError::internal(format!("system clock is before Unix epoch: {error}"))
            })?;
        let run_id = format!(
            "run-{}-{}-{}",
            stamp.as_secs(),
            stamp.subsec_nanos(),
            std::process::id()
        );
        let directory = if let Some(path) = requested {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AppError::internal(format!(
                        "cannot create log directory parent {}: {error}",
                        parent.display()
                    ))
                })?;
            }
            create_private_directory(path).map_err(|error| {
                AppError::internal(format!(
                    "cannot create log directory {}: {error}",
                    path.display()
                ))
            })?;
            path.to_path_buf()
        } else {
            location::default_run_directory(&run_id)?
        };
        let tasks = plan
            .tasks()
            .iter()
            .enumerate()
            .map(|(index, task)| {
                let file = format!(
                    "{:04}-{}.log",
                    index.saturating_add(1),
                    task_file_component(task.name())
                );
                (task.name().clone(), directory.join(file))
            })
            .collect();
        Ok(Self {
            run_id,
            directory,
            tasks,
        })
    }

    pub(super) fn start(
        &self,
        name: &TaskName,
        cwd: &Path,
        environment: impl Iterator<Item = String>,
        command: &[String],
    ) -> Result<TaskLog, AppError> {
        let path = self
            .tasks
            .get(name)
            .ok_or_else(|| AppError::internal(format!("task `{name}` has no reserved log path")))?;
        let mut file = create_private_file(path).map_err(|error| log_error(path, &error))?;
        let header = format!(
            "command: {:?}\ncwd: {}\nenvironment overrides: {:?}\n",
            command,
            cwd.display(),
            environment.collect::<Vec<_>>()
        );
        file.write_all(header.as_bytes())
            .map_err(|error| log_error(path, &error))?;
        Ok(TaskLog {
            path: path.clone(),
            file: Arc::new(Mutex::new(file)),
        })
    }

    pub(super) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(super) fn run_id(&self) -> &str {
        &self.run_id
    }
}

#[derive(Clone, Debug)]
pub(super) struct TaskLog {
    path: PathBuf,
    file: Arc<Mutex<File>>,
}

impl TaskLog {
    pub(super) fn write_stream(&self, label: &str, bytes: &[u8]) -> io::Result<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("task log lock is poisoned"))?;
        file.write_all(format!("\n[{label}]\n").as_bytes())?;
        file.write_all(bytes)
    }

    pub(super) fn write_launch_error(&self, error: &str) -> Result<(), AppError> {
        self.write_stream("launch error", error.as_bytes())
            .map_err(|error| log_error(&self.path, &error))
    }

    pub(super) fn finish(self) -> Result<PathBuf, AppError> {
        self.file
            .lock()
            .map_err(|_| AppError::internal("task log lock is poisoned"))?
            .sync_all()
            .map_err(|error| log_error(&self.path, &error))?;
        Ok(self.path)
    }
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> io::Result<()> {
    DirBuilder::new().create(path)
}

#[cfg(unix)]
pub(super) fn create_private_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
pub(super) fn create_private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn task_file_component(name: &TaskName) -> &str {
    let source = name.as_str();
    &source[..source.len().min(64)]
}

fn log_error(path: &Path, error: &io::Error) -> AppError {
    AppError::internal(format!("cannot write task log {}: {error}", path.display()))
}
