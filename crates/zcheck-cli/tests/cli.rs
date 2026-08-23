//! End-to-end inspection-command contracts for the published binary.

use std::path::{Path, PathBuf};
use std::process::Command;

use zcheck_testkit::RepositoryFixture;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(PathBuf::new, Path::to_path_buf)
}

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

#[test]
fn validates_the_canonical_repository_manifest() {
    let output = Command::new(binary())
        .current_dir(workspace())
        .arg("validate")
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stdout).contains("zcheck.toml: valid")
    }));
}

#[test]
fn emits_a_versioned_json_plan_for_the_default_task() {
    let output = Command::new(binary())
        .current_dir(workspace())
        .args(["plan", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    assert_eq!(
        report.as_ref().and_then(|value| value["schema"].as_u64()),
        Some(1)
    );
    assert_eq!(
        report
            .as_ref()
            .and_then(|value| value["selection"][0].as_str()),
        Some("check")
    );
}

#[test]
fn human_plans_quote_every_argument_boundary() {
    let fixture = RepositoryFixture::new(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
run = ["tool", "hello world", "--value=a b", "quote\"inside"]
"#,
    );
    let output = fixture.command(binary()).arg("plan").output().ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stdout)
            .contains(r#""tool" "hello world" "--value=a b" "quote\"inside""#)
    }));
}
