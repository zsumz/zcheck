//! Black-box contracts for GitHub reporting and failure artifact retention.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use zcheck_testkit::RepositoryFixture;

#[path = "github_reporter/fixture.rs"]
mod fixture;

const ROLE: &str = "ZCHECK_GITHUB_REPORTER_ROLE";

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

fn fixture_binary() -> PathBuf {
    let executable = env::current_exe();
    assert!(executable.is_ok(), "test executable must be available");
    executable.unwrap_or_default()
}

fn quoted(path: impl AsRef<Path>) -> String {
    format!("{:?}", path.as_ref().display().to_string())
}

fn failing_command() -> String {
    format!(
        "run = [{}, \"--exact\", {:?}, \"--nocapture\"]\nenv = {{ {ROLE} = \"fail\", RUST_BACKTRACE = \"0\" }}\n",
        quoted(fixture_binary()),
        fixture::TEST_NAME
    )
}

#[test]
fn github_groups_are_serial_and_failure_keeps_its_exit_and_artifacts() {
    let fixture = RepositoryFixture::new(&format!(
        "schema = 1\ndefault = \"check\"\n[execution]\nrepository_state = \"preserve\"\n[tasks.fail]\n{}[tasks.check]\nneeds = [\"fail\"]\n",
        failing_command()
    ));
    assert!(fixture.initialize_git());
    let output = fixture
        .command(binary())
        .args([
            "run",
            "--format",
            "github",
            "--logs-dir",
            "artifacts/logs",
            "--receipt",
            "artifacts/receipt.json",
        ])
        .output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(1),
        "{}",
        output.as_ref().map_or_else(
            |_| String::new(),
            |value| String::from_utf8_lossy(&value.stderr).into_owned()
        )
    );
    assert!(output.as_ref().is_ok_and(|value| value.stderr.is_empty()));
    let stdout = output.as_ref().map_or_else(
        |_| String::new(),
        |value| String::from_utf8_lossy(&value.stdout).into_owned(),
    );
    assert_serial_groups(&stdout);
    assert!(stdout.contains("::error title=zcheck failed%3A fail::"));
    assert!(stdout.lines().any(|line| {
        line.starts_with("| ") && line.contains("::error title=injected::payload%")
    }));
    assert!(
        !stdout
            .lines()
            .any(|line| line.starts_with("::error title=injected"))
    );

    let receipt_path = fixture.repository().join("artifacts/receipt.json");
    let receipt = fs::read_to_string(&receipt_path)
        .ok()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok());
    assert_eq!(
        receipt.as_ref().and_then(|value| value["status"].as_str()),
        Some("failed")
    );
    assert_eq!(
        receipt
            .as_ref()
            .and_then(|value| value["repository"]["preserved"].as_bool()),
        Some(true)
    );
    assert!(fixture.repository().join("artifacts/logs").is_dir());
}

#[test]
fn github_annotations_have_a_stable_run_wide_limit() {
    let mut manifest = String::from(
        "schema = 1\ndefault = \"check\"\n[execution]\nrepository_state = \"ignore\"\n",
    );
    let mut needs = Vec::new();
    for index in 0..12 {
        let name = format!("fail-{index}");
        manifest.push_str("[tasks.");
        manifest.push_str(&name);
        manifest.push_str("]\nrun = [\"missing-zcheck-tool-");
        manifest.push_str(&index.to_string());
        manifest.push_str("\"]\n");
        needs.push(format!("\"{name}\""));
    }
    manifest.push_str("[tasks.check]\nneeds = [");
    manifest.push_str(&needs.join(", "));
    manifest.push_str("]\n");
    let fixture = RepositoryFixture::new(&manifest);
    let output = fixture
        .command(binary())
        .args(["run", "--format", "github"])
        .output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(1)
    );
    let stdout = output.as_ref().map_or_else(
        |_| String::new(),
        |value| String::from_utf8_lossy(&value.stdout).into_owned(),
    );
    let annotations = stdout
        .lines()
        .filter(|line| line.starts_with("::error"))
        .collect::<Vec<_>>();
    assert_eq!(annotations.len(), 10);
    assert!(
        annotations
            .last()
            .is_some_and(|line| line.contains("zcheck additional failures"))
    );
}

fn assert_serial_groups(output: &str) {
    let mut open = false;
    let mut groups = 0;
    for line in output.lines() {
        if line.starts_with("::group::") {
            assert!(!open, "GitHub groups must never nest or overlap");
            open = true;
            groups += 1;
        } else if line == "::endgroup::" {
            assert!(open, "every GitHub group close must have an open group");
            open = false;
        }
    }
    assert!(!open, "the final GitHub group must close");
    assert_eq!(groups, 3, "two task groups and one summary are expected");
}
