//! Strict manifest and deterministic planning contracts for zcheck.

mod duration;
mod error;
mod manifest;
mod model;
mod name;
mod plan;
mod platform;
mod receipt;
mod validate;

#[cfg(test)]
#[path = "compatibility_test.rs"]
mod compatibility_test;

pub use duration::DurationSpec;
pub use error::Error;
pub use model::{ExecutionPolicy, Manifest, RepositoryStatePolicy, Task};
pub use name::TaskName;
pub use plan::{Plan, PlannedTask, PlannedTaskKind, PlatformApplicability};
pub use platform::Platform;
pub use receipt::{
    HostEvidence, ManifestEvidence, Receipt, RepositoryEvidence, RepositorySnapshot, RunStatus,
    RunnerEvidence, TaskResult, TaskStatus, Termination, TerminationReason,
};
