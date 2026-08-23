//! Small, explicit parser for the stable CLI surface.

use std::path::PathBuf;

use super::error::AppError;

mod run;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Format {
    #[default]
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum RunFormat {
    #[default]
    Human,
    Json,
    Github,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Args {
    pub(crate) location: Location,
    pub(crate) command: Command,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct Location {
    pub(crate) root: Option<PathBuf>,
    pub(crate) manifest: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Help,
    Version,
    Run {
        tasks: Vec<String>,
        format: RunFormat,
        passthrough: Vec<String>,
        logs_dir: Option<PathBuf>,
        receipt: Option<PathBuf>,
        jobs: Option<usize>,
        fail_fast: bool,
    },
    List {
        format: Format,
    },
    Plan {
        tasks: Vec<String>,
        format: Format,
    },
    Validate {
        format: Format,
    },
    Doctor {
        format: Format,
    },
}

pub(crate) fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Args, AppError> {
    let mut values = arguments.into_iter().peekable();
    let mut location = Location::default();
    while matches!(
        values.peek().map(String::as_str),
        Some("--root" | "--manifest")
    ) {
        let option = values
            .next()
            .ok_or_else(|| AppError::usage("missing location option"))?;
        let value = values
            .next()
            .ok_or_else(|| AppError::usage(format!("{option} requires a path")))?;
        match option.as_str() {
            "--root" => set_once(&mut location.root, PathBuf::from(value), "--root")?,
            "--manifest" => {
                set_once(&mut location.manifest, PathBuf::from(value), "--manifest")?;
            }
            _ => return Err(AppError::usage(format!("unknown option `{option}`"))),
        }
    }
    if location.root.is_some() && location.manifest.is_some() {
        return Err(AppError::usage(
            "--root and --manifest cannot be used together",
        ));
    }
    let command = match values.next().as_deref() {
        None => Command::Run {
            tasks: Vec::new(),
            format: RunFormat::Human,
            passthrough: Vec::new(),
            logs_dir: None,
            receipt: None,
            jobs: None,
            fail_fast: false,
        },
        Some("--help" | "-h" | "help") => {
            reject_remaining(values)?;
            Command::Help
        }
        Some("--version" | "-V" | "version") => {
            reject_remaining(values)?;
            Command::Version
        }
        Some("run") => run::parse(values)?,
        Some("list") => Command::List {
            format: parse_format_only(values)?,
        },
        Some("validate") => Command::Validate {
            format: parse_format_only(values)?,
        },
        Some("plan") => parse_plan(values)?,
        Some("doctor") => Command::Doctor {
            format: parse_format_only(values)?,
        },
        Some(other) => {
            return Err(AppError::usage(format!(
                "unknown command `{other}`; expected run, list, plan, validate, or doctor"
            )));
        }
    };
    Ok(Args { location, command })
}

fn reject_remaining(mut arguments: impl Iterator<Item = String>) -> Result<(), AppError> {
    if let Some(extra) = arguments.next() {
        return Err(AppError::usage(format!("unexpected argument `{extra}`")));
    }
    Ok(())
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<(), AppError> {
    if slot.is_some() {
        return Err(AppError::usage(format!(
            "{option} may be provided only once"
        )));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_plan(arguments: impl IntoIterator<Item = String>) -> Result<Command, AppError> {
    let mut tasks = Vec::new();
    let mut format = Format::Human;
    let mut values = arguments.into_iter();
    while let Some(value) = values.next() {
        if value == "--format" {
            let value = values
                .next()
                .ok_or_else(|| AppError::usage("--format requires human or json"))?;
            format = parse_format(&value)?;
        } else if value.starts_with('-') {
            return Err(AppError::usage(format!("unknown plan option `{value}`")));
        } else {
            tasks.push(value);
        }
    }
    Ok(Command::Plan { tasks, format })
}

fn parse_format_only(arguments: impl IntoIterator<Item = String>) -> Result<Format, AppError> {
    let mut values = arguments.into_iter();
    let Some(option) = values.next() else {
        return Ok(Format::Human);
    };
    if option != "--format" {
        return Err(AppError::usage(format!("unknown option `{option}`")));
    }
    let value = values
        .next()
        .ok_or_else(|| AppError::usage("--format requires human or json"))?;
    let format = parse_format(&value)?;
    if let Some(extra) = values.next() {
        return Err(AppError::usage(format!("unexpected argument `{extra}`")));
    }
    Ok(format)
}

fn parse_format(value: &str) -> Result<Format, AppError> {
    match value {
        "human" => Ok(Format::Human),
        "json" => Ok(Format::Json),
        _ => Err(AppError::usage(format!(
            "unknown format `{value}`; expected human or json"
        ))),
    }
}

fn parse_run_format(value: &str) -> Result<RunFormat, AppError> {
    match value {
        "human" => Ok(RunFormat::Human),
        "json" => Ok(RunFormat::Json),
        "github" => Ok(RunFormat::Github),
        _ => Err(AppError::usage(format!(
            "unknown format `{value}`; expected human, json, or github"
        ))),
    }
}

#[cfg(test)]
#[path = "args_test.rs"]
mod args_test;
