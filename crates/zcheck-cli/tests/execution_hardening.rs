//! Adversarial execution tests for qualification trust boundaries.

use std::path::PathBuf;

use zcheck_testkit::RepositoryFixture;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

fn quoted(value: &str) -> String {
    format!("{value:?}")
}

fn task_status(report: &serde_json::Value, name: &str) -> Option<String> {
    report["tasks"].as_array().and_then(|tasks| {
        tasks
            .iter()
            .find(|task| task["name"] == name)
            .and_then(|task| task["status"].as_str())
            .map(ToOwned::to_owned)
    })
}

fn task_log(report: &serde_json::Value, name: &str) -> Option<PathBuf> {
    report["tasks"].as_array().and_then(|tasks| {
        tasks
            .iter()
            .find(|task| task["name"] == name)
            .and_then(|task| task["log"].as_str())
            .map(PathBuf::from)
    })
}

#[test]
fn missing_working_directory_blocks_only_its_branch() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.bad]
cwd = "missing"
run = [{command}, "--version"]
[tasks.independent]
run = [{command}, "--version"]
[tasks.check]
needs = ["bad", "independent"]
"#
    ));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
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
        report.as_ref().and_then(|value| task_status(value, "bad")),
        Some("blocked".to_owned())
    );
    assert_eq!(
        report
            .as_ref()
            .and_then(|value| task_status(value, "independent")),
        Some("passed".to_owned())
    );
    assert_eq!(
        report
            .as_ref()
            .and_then(|value| task_status(value, "check")),
        Some("blocked".to_owned())
    );
}

#[cfg(unix)]
#[test]
fn manifest_symlink_may_not_escape_the_repository() {
    let fixture = RepositoryFixture::new("schema = 1\n[tasks.check]\nrun = [\"cargo\"]\n");
    let external = fixture.write_cache_file(
        "external.toml",
        "schema = 1\ndefault = \"check\"\n[tasks.check]\nrun = [\"cargo\", \"--version\"]\n",
    );
    assert!(
        external.as_ref().is_some_and(|path| {
            fixture.replace_repository_file_with_symlink(path, "zcheck.toml")
        })
    );
    let output = fixture.command(binary()).arg("validate").output().ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(2)
    );
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stderr).contains("resolves outside its repository root")
    }));
}

#[test]
fn long_task_names_use_bounded_unique_log_file_names() {
    let name = "a".repeat(252);
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        "schema = 1\ndefault = {name:?}\n[execution]\nrepository_state = \"ignore\"\n[tasks.{name}]\nrun = [{command}, \"--version\"]\n"
    ));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    let log = report.as_ref().and_then(|value| task_log(value, &name));
    assert!(log.as_ref().is_some_and(|path| {
        path.file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.len() < 100)
    }));
}

#[test]
fn platform_reserved_task_names_are_not_raw_log_file_names() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        "schema = 1\ndefault = \"con\"\n[execution]\nrepository_state = \"ignore\"\n[tasks.con]\nrun = [{command}, \"--version\"]\n"
    ));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    let log = report.as_ref().and_then(|value| task_log(value, "con"));
    assert!(log.as_ref().is_some_and(|path| {
        path.file_name().and_then(|value| value.to_str()) == Some("0001-con.log")
    }));
}

#[cfg(unix)]
#[test]
fn run_directories_and_task_logs_are_private() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        "schema = 1\ndefault = \"check\"\n[execution]\nrepository_state = \"ignore\"\n[tasks.check]\nrun = [{command}, \"--version\"]\n"
    ));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    let directory = report
        .as_ref()
        .and_then(|value| value["logs_dir"].as_str())
        .map(PathBuf::from);
    let log = report.as_ref().and_then(|value| task_log(value, "check"));
    assert_eq!(
        directory.as_ref().and_then(|path| fixture.unix_mode(path)),
        Some(0o700)
    );
    assert_eq!(
        log.as_ref().and_then(|path| fixture.unix_mode(path)),
        Some(0o600)
    );
}

#[cfg(unix)]
#[test]
fn relative_artifact_paths_may_not_follow_symlinks_outside_the_repository() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        "schema = 1\ndefault = \"check\"\n[execution]\nrepository_state = \"ignore\"\n[tasks.check]\nrun = [{command}, \"--version\"]\n"
    ));
    assert!(fixture.symlink_into_repository(fixture.cache(), "artifacts"));
    let output = fixture
        .command(binary())
        .args(["run", "--logs-dir", "artifacts/logs"])
        .output()
        .ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(2)
    );
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stderr).contains("resolves outside the repository")
    }));
    assert!(!fixture.cache().join("logs").exists());
}
