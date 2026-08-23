//! Bounded task-graph scheduling and evidence composition.

use zcheck_core::Plan;

use super::super::discovery::Repository;
use super::super::error::AppError;
use super::artifacts::ArtifactOptions;
use super::cancellation::Cancellation;
use super::evidence::Evidence;
use super::logs::RunLogs;
use super::model::{ExecutionOptions, RunReport};
use super::receipt_io;
use super::selection::validate_passthrough;

mod dag;
mod state;

pub(crate) fn execute(
    repository: &Repository,
    plan: &Plan,
    passthrough: &[String],
    artifacts: ArtifactOptions,
    options: ExecutionOptions,
) -> Result<RunReport, AppError> {
    validate_passthrough(plan, passthrough)?;
    let cancellation = Cancellation::install()?;
    let artifacts = artifacts.resolve(&repository.root)?;
    let logs = RunLogs::create(plan, artifacts.logs_dir.as_deref())?;
    let receipt_path = artifacts
        .receipt
        .unwrap_or_else(|| logs.directory().join("receipt.json"));
    let evidence = Evidence::begin(repository, &logs, receipt_path)?;
    let results = if let Some(failure) = evidence.initial_failure()? {
        vec![failure]
    } else {
        dag::execute_plan(repository, plan, passthrough, &logs, &cancellation, options)?
    };
    let receipt_path = evidence.receipt_path().to_path_buf();
    let receipt = evidence.finish(plan, &logs, results)?;
    receipt_io::write(&receipt, &receipt_path)?;
    let name = repository.root.file_name().map_or_else(
        || repository.root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    Ok(RunReport::new(name, receipt))
}
