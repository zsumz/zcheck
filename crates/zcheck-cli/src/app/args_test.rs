//! Tests for the explicit command-line grammar.

use std::path::PathBuf;

use super::{Command, Format, RunFormat, parse};

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

#[test]
fn parses_location_plan_selection_and_json_format() {
    let args = parse(strings(&[
        "--root", "repo", "plan", "lint", "test", "--format", "json",
    ]));
    let args = args.ok();
    assert_eq!(
        args.as_ref().and_then(|value| value.location.root.as_ref()),
        Some(&PathBuf::from("repo"))
    );
    assert!(matches!(
        args.as_ref().map(|value| &value.command),
        Some(Command::Plan { tasks, format: Format::Json }) if tasks == &strings(&["lint", "test"])
    ));
}

#[test]
fn parses_run_selection_format_and_passthrough_arguments() {
    let args = parse(strings(&[
        "run",
        "test",
        "--format",
        "json",
        "--",
        "--nocapture",
    ]));
    assert!(matches!(
        args.ok().map(|value| value.command),
        Some(Command::Run {
            tasks,
            format: RunFormat::Json,
            passthrough,
            ..
        }) if tasks == strings(&["test"]) && passthrough == strings(&["--nocapture"])
    ));
}

#[test]
fn bare_invocation_selects_the_default_run() {
    assert!(matches!(
        parse(Vec::new()).ok().map(|value| value.command),
        Some(Command::Run {
            tasks,
            format: RunFormat::Human,
            passthrough,
            logs_dir: None,
            receipt: None,
            jobs: None,
            fail_fast: false,
        }) if tasks.is_empty() && passthrough.is_empty()
    ));
}

#[test]
fn parses_run_scheduling_options_strictly() {
    let args = parse(strings(&["run", "check", "--jobs", "4", "--fail-fast"]));
    assert!(matches!(
        args.ok().map(|value| value.command),
        Some(Command::Run {
            jobs: Some(4),
            fail_fast: true,
            ..
        })
    ));
    for invalid in [
        strings(&["run", "--jobs", "0"]),
        strings(&["run", "--jobs", "many"]),
        strings(&["run", "--jobs", "2", "--jobs", "3"]),
        strings(&["run", "--fail-fast", "--fail-fast"]),
    ] {
        assert!(parse(invalid).is_err());
    }
}

#[test]
fn github_format_is_explicitly_scoped_to_task_runs() {
    let args = parse(strings(&["run", "check", "--format", "github"]));
    assert!(matches!(
        args.ok().map(|value| value.command),
        Some(Command::Run {
            format: RunFormat::Github,
            ..
        })
    ));
    assert!(parse(strings(&["plan", "check", "--format", "github"])).is_err());
    assert!(parse(strings(&["doctor", "--format", "github"])).is_err());
}

#[test]
fn parses_explicit_run_artifact_paths_once() {
    let args = parse(strings(&[
        "run",
        "check",
        "--logs-dir",
        "artifacts/logs",
        "--receipt",
        "artifacts/receipt.json",
    ]));
    assert!(matches!(
        args.ok().map(|value| value.command),
        Some(Command::Run { logs_dir: Some(logs), receipt: Some(receipt), .. })
            if logs.to_str() == Some("artifacts/logs")
                && receipt.to_str() == Some("artifacts/receipt.json")
    ));
    assert!(
        parse(strings(&[
            "run",
            "--receipt",
            "one.json",
            "--receipt",
            "two.json"
        ]))
        .is_err()
    );
}

#[test]
fn rejects_ambiguous_locations_and_unknown_options() {
    assert!(
        parse(strings(&[
            "--root",
            ".",
            "--manifest",
            "zcheck.toml",
            "list"
        ]))
        .is_err()
    );
    assert!(parse(strings(&["plan", "check", "--jobs", "4"])).is_err());
    assert!(parse(strings(&["--version", "extra"])).is_err());
}
