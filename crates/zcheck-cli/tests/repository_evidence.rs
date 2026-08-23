//! Adversarial contracts for exact checkout-state evidence and receipts.

use std::fs;

use zcheck_testkit::RepositoryFixture;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

fn quoted(value: &str) -> String {
    format!("{value:?}")
}

fn report(output: &std::process::Output) -> Option<serde_json::Value> {
    serde_json::from_slice(&output.stdout).ok()
}

fn tracked_fixture(policy: &str, command: &str) -> RepositoryFixture {
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "{policy}"
[tasks.check]
run = {command}
tools = ["git"]
env = {{ RECEIPT_SECRET = "must-not-appear" }}
"#
    ));
    assert!(fixture.write_repository_file("tracked.txt", "clean\n"));
    assert!(fixture.write_repository_file(
        "mutation.patch",
        "diff --git a/tracked.txt b/tracked.txt\n--- a/tracked.txt\n+++ b/tracked.txt\n@@ -1 +1 @@\n-one dirty\n+two dirty\n",
    ));
    assert!(fixture.initialize_git());
    fixture
}

#[test]
fn preserve_detects_content_changes_hidden_by_the_same_status_code() {
    let fixture = tracked_fixture("preserve", r#"["git", "apply", "mutation.patch"]"#);
    assert!(fixture.write_repository_file("tracked.txt", "one dirty\n"));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(1)
    );
    let receipt = output.as_ref().ok().and_then(report);
    assert_eq!(
        receipt.as_ref().and_then(|value| value["status"].as_str()),
        Some("failed")
    );
    assert_eq!(
        receipt
            .as_ref()
            .and_then(|value| value["repository"]["preserved"].as_bool()),
        Some(false)
    );
    assert!(receipt.as_ref().is_some_and(|value| {
        value["tasks"].as_array().is_some_and(|tasks| {
            tasks.iter().any(|task| {
                task["name"] == "repository-state"
                    && task["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("tracked.txt"))
            })
        })
    }));
    assert!(output.as_ref().is_ok_and(|value| {
        !String::from_utf8_lossy(&value.stdout).contains("must-not-appear")
    }));
}

#[test]
fn preserve_accepts_an_exactly_unchanged_dirty_checkout() {
    let command = format!("[{}, \"--version\"]", quoted(binary()));
    let fixture = tracked_fixture("preserve", &command);
    assert!(fixture.write_repository_file("tracked.txt", "one dirty\n"));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert!(output.as_ref().is_ok_and(|value| value.status.success()));
    let receipt = output.as_ref().ok().and_then(report);
    assert_eq!(
        receipt
            .as_ref()
            .and_then(|value| value["repository"]["preserved"].as_bool()),
        Some(true)
    );
    assert_eq!(
        receipt
            .as_ref()
            .and_then(|value| value["runner"]["version"].as_str()),
        Some("0.0.1")
    );
}

#[test]
fn clean_rejects_initial_dirt_without_starting_tasks() {
    let fixture = tracked_fixture("clean", r#"["git", "apply", "mutation.patch"]"#);
    assert!(fixture.write_repository_file("tracked.txt", "one dirty\n"));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(1)
    );
    assert_eq!(
        fs::read_to_string(fixture.repository().join("tracked.txt"))
            .ok()
            .as_deref(),
        Some("one dirty\n")
    );
    let receipt = output.as_ref().ok().and_then(report);
    assert_eq!(
        receipt
            .as_ref()
            .and_then(|value| value["tasks"].as_array())
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn explicit_artifacts_are_persisted_without_failing_preservation() {
    let command = format!("[{}, \"--version\"]", quoted(binary()));
    let fixture = tracked_fixture("preserve", &command);
    let output = fixture
        .command(binary())
        .args([
            "run",
            "--format",
            "json",
            "--logs-dir",
            "artifacts/logs",
            "--receipt",
            "artifacts/receipt.json",
        ])
        .output();
    assert!(output.as_ref().is_ok_and(|value| value.status.success()));
    let receipt_path = fixture.repository().join("artifacts/receipt.json");
    let persisted = fs::read(&receipt_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    assert_eq!(
        persisted
            .as_ref()
            .and_then(|value| value["repository"]["preserved"].as_bool()),
        Some(true)
    );
    assert!(fixture.repository().join("artifacts/logs").is_dir());
}
