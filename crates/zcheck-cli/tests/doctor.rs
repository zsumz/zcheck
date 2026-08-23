//! End-to-end doctor contracts for optional repository capabilities.

use zcheck_testkit::RepositoryFixture;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

#[test]
fn ignore_repository_state_does_not_require_git() {
    let fixture = RepositoryFixture::new(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
run = ["zcheck-doctor-task", "--version"]
"#,
    );
    let tool = if cfg!(windows) {
        "zcheck-doctor-task.exe"
    } else {
        "zcheck-doctor-task"
    };
    assert!(fixture.copy_to_cache(binary(), tool).is_some());
    let output = fixture
        .command(binary())
        .env("PATH", fixture.cache())
        .args(["doctor", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    assert_eq!(
        report.as_ref().and_then(|value| value["status"].as_str()),
        Some("passed")
    );
    assert!(report.as_ref().is_some_and(|value| {
        value["checks"].as_array().is_some_and(|checks| {
            checks.iter().all(|check| check["name"] != "git")
                && checks
                    .iter()
                    .any(|check| check["name"] == "repository-state" && check["status"] == "passed")
                && checks
                    .iter()
                    .any(|check| check["name"] == "default-task" && check["status"] == "passed")
        })
    }));
}
