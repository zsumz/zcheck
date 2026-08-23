//! End-to-end task execution contracts for the published binary.

use std::path::PathBuf;

use zcheck_testkit::RepositoryFixture;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

fn quoted(value: &str) -> String {
    format!("{value:?}")
}

fn task<'a>(report: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    report["tasks"]
        .as_array()
        .and_then(|tasks| tasks.iter().find(|task| task["name"] == name))
}

#[test]
fn bare_invocation_runs_the_default_graph() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.leaf]
run = [{command}, "--version"]
[tasks.check]
needs = ["leaf"]
"#
    ));
    let output = fixture.command(binary()).output().ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let stdout = output.as_ref().map_or_else(String::new, |value| {
        String::from_utf8_lossy(&value.stdout).into_owned()
    });
    assert!(stdout.contains("PASS   leaf"));
    assert!(stdout.contains("qualification passed"));
    let explicit = fixture.command(binary()).arg("run").output().ok();
    assert!(
        explicit
            .as_ref()
            .is_some_and(|value| value.status.success())
    );
}

#[test]
fn failure_blocks_descendants_but_not_independent_tasks() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.fail]
run = [{command}, "not-a-command"]
[tasks.descendant]
needs = ["fail"]
run = [{command}, "--version"]
[tasks.independent]
run = [{command}, "--version"]
[tasks.check]
needs = ["descendant", "independent"]
"#
    ));
    let output = fixture
        .command(binary())
        .args(["run", "check", "--format", "json"])
        .output()
        .ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(1)
    );
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    for (name, status) in [
        ("fail", "failed"),
        ("descendant", "blocked"),
        ("independent", "passed"),
        ("check", "failed"),
    ] {
        assert_eq!(
            report
                .as_ref()
                .and_then(|value| task(value, name))
                .and_then(|result| result["status"].as_str()),
            Some(status)
        );
    }
    assert!(
        report
            .as_ref()
            .and_then(|value| task(value, "fail"))
            .and_then(|result| result["log"].as_str())
            .and_then(|path| fixture.read_to_string(path))
            .is_some_and(|contents| contents.contains("unknown command"))
    );
}

#[test]
fn doctor_blocks_when_a_required_tool_is_missing() {
    let fixture = RepositoryFixture::new(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
run = ["zcheck-tool-that-does-not-exist"]
"#,
    );
    let output = fixture
        .command(binary())
        .args(["doctor", "--format", "json"])
        .output()
        .ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(1)
    );
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    assert_eq!(
        report.as_ref().and_then(|value| value["status"].as_str()),
        Some("blocked")
    );
    assert!(report.as_ref().is_some_and(|value| {
        value["checks"].as_array().is_some_and(|checks| {
            checks.iter().any(|check| {
                check["name"] == "default-task"
                    && check["detail"]
                        .as_str()
                        .is_some_and(|detail| detail.contains("zcheck-tool-that-does-not-exist"))
            })
        })
    }));
}

#[test]
fn pass_through_arguments_reach_one_executable_root_and_its_log() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "inspect"
[execution]
repository_state = "ignore"
[tasks.inspect]
run = [{command}, "validate"]
"#
    ));
    let output = fixture
        .command(binary())
        .args([
            "run", "inspect", "--format", "json", "--", "--format", "json",
        ])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    let log = report
        .as_ref()
        .and_then(|value| task(value, "inspect"))
        .and_then(|result| result["log"].as_str())
        .map(PathBuf::from);
    assert!(
        log.as_ref()
            .is_some_and(|path| path.starts_with(fixture.cache()))
    );
    assert!(
        log.as_ref()
            .and_then(|path| fixture.read_to_string(path))
            .is_some_and(|contents| contents.contains(r#""status": "valid""#))
    );
}

#[test]
fn aggregate_tasks_reject_process_arguments_with_usage_exit() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[tasks.leaf]
run = [{command}, "--version"]
[tasks.check]
needs = ["leaf"]
"#
    ));
    let output = fixture
        .command(binary())
        .args(["run", "check", "--", "--extra"])
        .output()
        .ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(2)
    );
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stderr).contains("is an aggregate")
    }));
}

#[cfg(unix)]
#[test]
fn working_directory_and_environment_overrides_reach_processes() {
    let fixture = RepositoryFixture::new(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.cwd]
cwd = "nested"
run = ["pwd"]
[tasks.environment]
run = ["printenv", "ZCHECK_TEST_MARKER"]
env = { ZCHECK_TEST_MARKER = "present" }
[tasks.check]
needs = ["cwd", "environment"]
"#,
    );
    assert!(fixture.create_repository_directory("nested"));
    let output = fixture
        .command(binary())
        .args(["run", "check", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    let cwd_log = report
        .as_ref()
        .and_then(|value| task(value, "cwd"))
        .and_then(|result| result["log"].as_str())
        .and_then(|path| fixture.read_to_string(path));
    let env_log = report
        .as_ref()
        .and_then(|value| task(value, "environment"))
        .and_then(|result| result["log"].as_str())
        .and_then(|path| fixture.read_to_string(path));
    assert!(cwd_log.as_ref().is_some_and(|contents| {
        contents.contains(&fixture.repository().join("nested").display().to_string())
    }));
    assert!(
        env_log
            .as_ref()
            .is_some_and(|contents| contents.contains("present"))
    );
    assert!(env_log.as_ref().is_some_and(|contents| {
        !contents.contains("environment overrides: [\"ZCHECK_TEST_MARKER=present\"]")
    }));
}
