//! Repository-state enforcement and schema-1 receipt assembly.

use std::fs;
use std::path::{Path, PathBuf};

use zcheck_core::{
    HostEvidence, ManifestEvidence, Plan, Receipt, RepositoryEvidence, RepositoryStatePolicy,
    RunStatus, RunnerEvidence, TaskName, TaskResult, TaskStatus,
};

use super::super::discovery::Repository;
use super::super::error::AppError;
use super::logs::RunLogs;
use super::repository::{self, CapturedState};

pub(super) struct Evidence {
    policy: RepositoryStatePolicy,
    root: PathBuf,
    excluded: Vec<PathBuf>,
    receipt_path: PathBuf,
    manifest: ManifestEvidence,
    before: Option<CapturedState>,
}

impl Evidence {
    pub(super) fn begin(
        repository: &Repository,
        logs: &RunLogs,
        receipt_path: PathBuf,
    ) -> Result<Self, AppError> {
        let policy = repository.manifest.execution().repository_state();
        let excluded = repository_exclusions(&repository.root, logs.directory())?;
        let before = capture(policy, &repository.root, &excluded)?;
        Ok(Self {
            policy,
            root: repository.root.clone(),
            excluded,
            receipt_path,
            manifest: manifest_evidence(repository)?,
            before,
        })
    }

    pub(super) fn initial_failure(&self) -> Result<Option<TaskResult>, AppError> {
        if self.policy == RepositoryStatePolicy::Clean
            && self
                .before
                .as_ref()
                .is_some_and(|value| !value.evidence().clean)
        {
            return repository_failure("repository must be clean before qualification".to_owned())
                .map(Some);
        }
        Ok(None)
    }

    pub(super) fn finish(
        self,
        plan: &Plan,
        logs: &RunLogs,
        mut tasks: Vec<TaskResult>,
    ) -> Result<Receipt, AppError> {
        let after = capture(self.policy, &self.root, &self.excluded)?;
        let preserved = self
            .before
            .as_ref()
            .zip(after.as_ref())
            .map(|(left, right)| left.preserved(right));
        if preserved == Some(false) {
            let changes = self
                .before
                .as_ref()
                .zip(after.as_ref())
                .map_or_else(Vec::new, |(left, right)| left.changes(right));
            tasks.push(repository_failure(change_reason(&changes))?);
        }
        let status = if tasks
            .iter()
            .any(|task| task.status == TaskStatus::Cancelled)
        {
            RunStatus::Cancelled
        } else if tasks.iter().any(|task| task.status.fails_run()) {
            RunStatus::Failed
        } else {
            RunStatus::Passed
        };
        Ok(Receipt {
            schema: Receipt::SCHEMA,
            run_id: logs.run_id().to_owned(),
            runner: RunnerEvidence {
                name: "zcheck".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            manifest: self.manifest,
            repository: RepositoryEvidence {
                state_mode: self.policy,
                before: self.before.map(|state| state.evidence().clone()),
                after: after.map(|state| state.evidence().clone()),
                preserved,
            },
            selection: plan.selection().to_vec(),
            host: HostEvidence {
                os: plan.platform(),
                arch: std::env::consts::ARCH.to_owned(),
            },
            status,
            logs_dir: logs.directory().display().to_string(),
            receipt: self.receipt_path.display().to_string(),
            tasks,
        })
    }

    pub(super) fn receipt_path(&self) -> &Path {
        &self.receipt_path
    }
}

fn capture(
    policy: RepositoryStatePolicy,
    root: &Path,
    excluded: &[PathBuf],
) -> Result<Option<CapturedState>, AppError> {
    if policy == RepositoryStatePolicy::Ignore {
        Ok(None)
    } else {
        repository::capture(root, excluded).map(Some)
    }
}

fn repository_exclusions(root: &Path, logs: &Path) -> Result<Vec<PathBuf>, AppError> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        AppError::configuration(format!("cannot resolve repository root: {error}"))
    })?;
    let canonical_logs = fs::canonicalize(logs)
        .map_err(|error| AppError::internal(format!("cannot resolve task logs: {error}")))?;
    Ok(canonical_logs
        .strip_prefix(canonical_root)
        .ok()
        .map(Path::to_path_buf)
        .into_iter()
        .collect())
}

fn manifest_evidence(repository: &Repository) -> Result<ManifestEvidence, AppError> {
    let bytes = fs::read(&repository.manifest_path).map_err(|error| {
        AppError::internal(format!(
            "cannot hash manifest {}: {error}",
            repository.manifest_path.display()
        ))
    })?;
    let path = repository
        .manifest_path
        .strip_prefix(&repository.root)
        .map_or_else(
            |_| repository.manifest_path.display().to_string(),
            |relative| relative.display().to_string(),
        );
    Ok(ManifestEvidence {
        path,
        sha256: repository::digest(&bytes),
    })
}

fn repository_failure(reason: String) -> Result<TaskResult, AppError> {
    Ok(TaskResult {
        name: "repository-state"
            .parse::<TaskName>()
            .map_err(AppError::from)?,
        status: TaskStatus::Failed,
        command: None,
        cwd: ".".to_owned(),
        duration_ms: 0,
        exit_code: None,
        log: None,
        reason: Some(reason),
        environment: Vec::new(),
        termination: None,
    })
}

fn change_reason(changes: &[String]) -> String {
    const LIMIT: usize = 20;
    let mut reason = "qualification modified the checkout".to_owned();
    for change in changes.iter().take(LIMIT) {
        reason.push_str("; ");
        reason.push_str(change);
    }
    if changes.len() > LIMIT {
        reason.push_str("; and ");
        reason.push_str(&(changes.len() - LIMIT).to_string());
        reason.push_str(" more");
    }
    reason
}
