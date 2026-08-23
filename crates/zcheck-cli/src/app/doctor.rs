//! Current-checkout diagnostics for the configured default graph.

use serde::Serialize;
use zcheck_core::{Plan, RepositoryStatePolicy};

use super::args::Format;
use super::discovery::Repository;
use super::error::AppError;
use super::execution::{blocked_tasks, find_tool, inspect_repository_state};
use super::exit;

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: &'static str,
    status: &'static str,
    detail: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DoctorReport {
    schema: u32,
    version: &'static str,
    repository: String,
    manifest: String,
    default_task: String,
    status: &'static str,
    checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub(crate) fn exit_code(&self) -> i32 {
        if self.status == "passed" {
            exit::PASSED
        } else {
            exit::QUALIFICATION_FAILED
        }
    }
}

pub(crate) fn inspect(repository: &Repository, plan: &Plan) -> Result<DoctorReport, AppError> {
    let mut checks = vec![DoctorCheck {
        name: "manifest",
        status: "passed",
        detail: format!(
            "schema {} parsed and validated",
            repository.manifest.schema()
        ),
    }];
    let state = repository.manifest.execution().repository_state();
    let git = if state == RepositoryStatePolicy::Ignore {
        None
    } else {
        let git = find_tool(&repository.root, "git");
        checks.push(DoctorCheck {
            name: "git",
            status: if git.is_some() { "passed" } else { "blocked" },
            detail: git.as_ref().map_or_else(
                || "executable not found on PATH".to_owned(),
                |path| path.display().to_string(),
            ),
        });
        git
    };
    let state_check = if git.is_none() && state != RepositoryStatePolicy::Ignore {
        (
            false,
            "Git is required for repository-state enforcement".to_owned(),
        )
    } else {
        inspect_repository_state(&repository.root, state)
            .unwrap_or_else(|error| (false, error.to_string()))
    };
    checks.push(DoctorCheck {
        name: "repository-state",
        status: if state_check.0 { "passed" } else { "blocked" },
        detail: format!("{}: {}", repository_state_name(state), state_check.1),
    });
    if plan.tasks().iter().any(|task| task.timeout().is_some()) {
        checks.push(DoctorCheck {
            name: "timeouts",
            status: "passed",
            detail: "task timeouts terminate complete process trees".to_owned(),
        });
    }
    checks.push(DoctorCheck {
        name: "jobs",
        status: "passed",
        detail: format!(
            "up to {} executable task(s), with named resources exclusive",
            plan.jobs()
        ),
    });
    let blocked = blocked_tasks(&repository.root, &repository.manifest, plan)?;
    checks.push(DoctorCheck {
        name: "default-task",
        status: if blocked.is_empty() {
            "passed"
        } else {
            "blocked"
        },
        detail: if blocked.is_empty() {
            format!("{} is runnable", plan.selection()[0])
        } else {
            blocked
                .iter()
                .map(|(task, reason)| format!("{task}: {reason}"))
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    let passed = checks.iter().all(|check| check.status != "blocked");
    Ok(DoctorReport {
        schema: 1,
        version: env!("CARGO_PKG_VERSION"),
        repository: repository.root.display().to_string(),
        manifest: repository.manifest_path.display().to_string(),
        default_task: plan.selection()[0].to_string(),
        status: if passed { "passed" } else { "blocked" },
        checks,
    })
}

const fn repository_state_name(policy: RepositoryStatePolicy) -> &'static str {
    match policy {
        RepositoryStatePolicy::Preserve => "preserve",
        RepositoryStatePolicy::Clean => "clean",
        RepositoryStatePolicy::Ignore => "ignore",
    }
}

pub(crate) fn render(report: &DoctorReport, format: Format) -> Result<String, AppError> {
    if format == Format::Json {
        return serde_json::to_string_pretty(report)
            .map_err(|error| AppError::internal(format!("cannot render JSON: {error}")));
    }
    let mut lines = vec![format!(
        "zcheck doctor · {} · {}",
        report.version, report.default_task
    )];
    for check in &report.checks {
        lines.push(format!(
            "{:<6} {:<16} {}",
            check.status.to_uppercase(),
            check.name,
            check.detail
        ));
    }
    Ok(lines.join("\n"))
}
