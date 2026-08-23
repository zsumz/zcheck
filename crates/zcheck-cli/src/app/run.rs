//! Top-level command dispatch and stable process exit behavior.

use zcheck_core::{Platform, TaskName};

use super::args::{Command, parse};
use super::discovery;
use super::doctor;
use super::error::AppError;
use super::execution;
use super::exit;
use super::render;

const HELP: &str = r"zcheck - deterministic repository qualification graphs

USAGE
  zcheck [--root ROOT | --manifest PATH]
  zcheck [--root ROOT | --manifest PATH] run [TASK...] [--format human|json|github]
         [--jobs N] [--fail-fast] [--logs-dir PATH] [--receipt PATH] [-- ARGS...]
  zcheck [--root ROOT | --manifest PATH] list [--format human|json]
  zcheck [--root ROOT | --manifest PATH] plan [TASK...] [--format human|json]
  zcheck [--root ROOT | --manifest PATH] validate [--format human|json]
  zcheck [--root ROOT | --manifest PATH] doctor [--format human|json]

Bare zcheck and zcheck run execute the manifest's default task.
Explicit --logs-dir and --receipt paths must not already exist.
";

#[derive(Debug)]
struct Outcome {
    output: String,
    exit_code: i32,
}

impl Outcome {
    fn passed(output: String) -> Self {
        Self {
            output,
            exit_code: exit::PASSED,
        }
    }
}

pub(crate) fn run() -> i32 {
    match execute(std::env::args().skip(1)) {
        Ok(outcome) => {
            if !outcome.output.is_empty() {
                println!("{}", outcome.output);
            }
            outcome.exit_code
        }
        Err(error) => {
            eprintln!("error: {error}");
            error.exit_code()
        }
    }
}

fn execute(arguments: impl IntoIterator<Item = String>) -> Result<Outcome, AppError> {
    let args = parse(arguments)?;
    match args.command {
        Command::Help => Ok(Outcome::passed(HELP.to_owned())),
        Command::Version => Ok(Outcome::passed(format!(
            "zcheck {}",
            env!("CARGO_PKG_VERSION")
        ))),
        Command::Run {
            tasks,
            format,
            passthrough,
            logs_dir,
            receipt,
            jobs,
            fail_fast,
        } => {
            let repository = discovery::load(&args.location)?;
            let selection = select(&repository.manifest, &tasks)?;
            let plan = repository.manifest.plan(&selection, current_platform()?)?;
            let report = execution::execute(
                &repository,
                &plan,
                &passthrough,
                execution::ArtifactOptions { logs_dir, receipt },
                execution::ExecutionOptions {
                    jobs,
                    fail_fast,
                    progress: format == super::args::RunFormat::Human,
                },
            )?;
            let exit_code = report.exit_code();
            Ok(Outcome {
                output: execution::render(&report, format)?,
                exit_code,
            })
        }
        Command::List { format } => {
            let repository = discovery::load(&args.location)?;
            Ok(Outcome::passed(render::list(&repository.manifest, format)?))
        }
        Command::Validate { format } => {
            let repository = discovery::load(&args.location)?;
            Ok(Outcome::passed(render::validation(
                &repository.manifest,
                &display_path(&repository),
                format,
            )?))
        }
        Command::Plan { tasks, format } => {
            let repository = discovery::load(&args.location)?;
            let selection = select(&repository.manifest, &tasks)?;
            let plan = repository.manifest.plan(&selection, current_platform()?)?;
            Ok(Outcome::passed(render::plan(&plan, format)?))
        }
        Command::Doctor { format } => {
            let repository = discovery::load(&args.location)?;
            let selection = select(&repository.manifest, &[])?;
            let plan = repository.manifest.plan(&selection, current_platform()?)?;
            let report = doctor::inspect(&repository, &plan)?;
            Ok(Outcome {
                output: doctor::render(&report, format)?,
                exit_code: report.exit_code(),
            })
        }
    }
}

fn current_platform() -> Result<Platform, AppError> {
    Platform::current().ok_or_else(|| {
        AppError::internal("this zcheck build does not support the current operating system")
    })
}

fn select(manifest: &zcheck_core::Manifest, tasks: &[String]) -> Result<Vec<TaskName>, AppError> {
    if tasks.is_empty() {
        return manifest
            .default_task()
            .cloned()
            .map(|task| vec![task])
            .ok_or_else(|| {
                AppError::configuration(
                    "no tasks selected and the manifest does not define a default task",
                )
            });
    }
    tasks
        .iter()
        .map(|name| name.parse::<TaskName>().map_err(AppError::from))
        .collect()
}

fn display_path(repository: &discovery::Repository) -> String {
    repository
        .manifest_path
        .strip_prefix(&repository.root)
        .map_or_else(
            |_| repository.manifest_path.display().to_string(),
            |path| path.display().to_string(),
        )
}
