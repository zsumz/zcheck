//! Stable schema-1 evidence receipts for completed qualification runs.

use serde::{Deserialize, Serialize};

use crate::{Error, Platform, RepositoryStatePolicy, TaskName};

/// Complete machine-readable evidence for one zcheck invocation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    /// Receipt schema version.
    pub schema: u32,
    /// Invocation identifier shared by its logs and receipt.
    pub run_id: String,
    /// Runner identity.
    pub runner: RunnerEvidence,
    /// Canonical manifest identity.
    pub manifest: ManifestEvidence,
    /// Git checkout evidence.
    pub repository: RepositoryEvidence,
    /// Explicitly selected root tasks.
    pub selection: Vec<TaskName>,
    /// Host operating-system and architecture identity.
    pub host: HostEvidence,
    /// Overall qualification status.
    pub status: RunStatus,
    /// Directory containing complete task logs.
    pub logs_dir: String,
    /// Path at which this receipt is persisted.
    pub receipt: String,
    /// Results for every planned task and synthetic enforcement task.
    pub tasks: Vec<TaskResult>,
}

impl Receipt {
    /// Receipt JSON schema version accepted and emitted by this release line.
    pub const SCHEMA: u32 = 1;

    /// Parses one complete schema-1 receipt and rejects unknown fields.
    pub fn parse_json(source: &str) -> Result<Self, Error> {
        let receipt = serde_json::from_str::<Self>(source)
            .map_err(|error| Error::new(format!("invalid zcheck receipt: {error}")))?;
        if receipt.schema != Self::SCHEMA {
            return Err(Error::new(format!(
                "unsupported receipt schema {}; this zcheck supports schema {}",
                receipt.schema,
                Self::SCHEMA
            )));
        }
        Ok(receipt)
    }

    /// Serializes this receipt using the stable pretty-printed JSON contract.
    pub fn to_json_pretty(&self) -> Result<String, Error> {
        serde_json::to_string_pretty(self)
            .map_err(|error| Error::new(format!("cannot serialize receipt: {error}")))
    }
}

/// Name and version of the runner that produced a receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerEvidence {
    /// Stable runner name.
    pub name: String,
    /// Exact runner package version.
    pub version: String,
}

/// Path and content identity of the parsed manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestEvidence {
    /// Repository-relative manifest path.
    pub path: String,
    /// Lowercase SHA-256 digest of the manifest bytes.
    pub sha256: String,
}

/// Git policy and before/after state for one checkout.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryEvidence {
    /// Requested checkout-state policy.
    pub state_mode: RepositoryStatePolicy,
    /// State captured before execution, absent only for `ignore`.
    pub before: Option<RepositorySnapshot>,
    /// State captured after execution, absent only for `ignore`.
    pub after: Option<RepositorySnapshot>,
    /// Exact preservation outcome, absent only for `ignore`.
    pub preserved: Option<bool>,
}

/// Content-correct fingerprint components for one Git checkout state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepositorySnapshot {
    /// Combined SHA-256 fingerprint of every component.
    pub sha256: String,
    /// Current `HEAD` object identity, or `None` for an unborn branch.
    pub head: Option<String>,
    /// SHA-256 of the staged binary diff.
    pub staged_sha256: String,
    /// SHA-256 of the unstaged binary diff.
    pub unstaged_sha256: String,
    /// SHA-256 of untracked paths, file contents, and symlink targets.
    pub untracked_sha256: String,
    /// SHA-256 of recursive submodule state.
    pub submodules_sha256: String,
    /// Whether Git state is completely clean.
    pub clean: bool,
}

/// Host projection recorded by a receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostEvidence {
    /// Supported operating-system family.
    pub os: Platform,
    /// Rust target architecture spelling.
    pub arch: String,
}

/// Stable overall run status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    /// Every applicable result passed.
    Passed,
    /// At least one result failed or was blocked.
    Failed,
    /// The user interrupted the invocation.
    Cancelled,
}

/// Stable result status for one planned or synthetic task.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// The task completed successfully.
    Passed,
    /// The task process or enforcement check failed.
    Failed,
    /// The task could not run safely.
    Blocked,
    /// The task did not apply to this platform.
    Skipped,
    /// The invocation was interrupted before the task completed.
    Cancelled,
}

impl TaskStatus {
    /// Returns whether this status makes qualification unsuccessful.
    #[must_use]
    pub const fn fails_run(self) -> bool {
        matches!(self, Self::Failed | Self::Blocked | Self::Cancelled)
    }
}

/// Evidence for one planned task or synthetic enforcement result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskResult {
    /// Task identity.
    pub name: TaskName,
    /// Stable result status.
    pub status: TaskStatus,
    /// Executed direct argument vector, when applicable.
    pub command: Option<Vec<String>>,
    /// Repository-relative working directory.
    pub cwd: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Direct child exit code, when available.
    pub exit_code: Option<i32>,
    /// Complete task-log location, when applicable.
    pub log: Option<String>,
    /// Bounded explanation for a non-passing result.
    pub reason: Option<String>,
    /// Names of explicit environment overrides; values are never recorded.
    pub environment: Vec<String>,
    /// Timeout or interruption details, when zcheck terminated the task.
    pub termination: Option<Termination>,
}

/// Why and how zcheck terminated a task process tree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Termination {
    /// Trigger for termination.
    pub reason: TerminationReason,
    /// Whether escalation to forced termination was required.
    pub forced: bool,
}

/// Stable process-tree termination reason.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminationReason {
    /// The configured task timeout expired.
    Timeout,
    /// The user interrupted the zcheck invocation.
    Interrupted,
}

#[cfg(test)]
#[path = "receipt_test.rs"]
mod receipt_test;
