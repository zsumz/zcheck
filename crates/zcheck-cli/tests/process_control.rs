//! Adversarial process-tree, timeout, interruption, and output-capture contracts.

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use command_group::{Signal, UnixChildExt};
use zcheck_testkit::RepositoryFixture;

#[path = "process_control/fixture.rs"]
mod fixture;

const ROLE: &str = "ZCHECK_PROCESS_FIXTURE_ROLE";
const HEARTBEAT: &str = "ZCHECK_PROCESS_FIXTURE_HEARTBEAT";
const LATER_MARKER: &str = "ZCHECK_PROCESS_FIXTURE_LATER";

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

fn quoted(value: impl AsRef<Path>) -> String {
    format!("{:?}", value.as_ref().display().to_string())
}

fn fixture_binary() -> PathBuf {
    let executable = std::env::current_exe();
    assert!(
        executable.is_ok(),
        "process-control test executable must be available"
    );
    executable.unwrap_or_default()
}

fn manifest(role: &str, timeout: &str, heartbeat: &Path) -> String {
    let helper = quoted(fixture_binary());
    let heartbeat = quoted(heartbeat);
    let test_name = fixture::TEST_NAME;
    format!(
        r#"schema = 1
default = "task"
[execution]
repository_state = "ignore"
default_timeout = {timeout:?}
[tasks.task]
run = [{helper}, "--exact", {test_name:?}, "--nocapture"]
env = {{ {ROLE} = {role:?}, {HEARTBEAT} = {heartbeat} }}
"#
    )
}

fn parse(output: &std::process::Output) -> Option<serde_json::Value> {
    serde_json::from_slice(&output.stdout).ok()
}

fn task<'a>(receipt: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    receipt["tasks"]
        .as_array()?
        .iter()
        .find(|task| task["name"] == name)
}

fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    predicate()
}

#[test]
fn large_stdout_and_stderr_stream_without_deadlock_or_memory_buffering() {
    let placeholder = Path::new("unused");
    let manifest = manifest("large-output", "10s", placeholder);
    let fixture = RepositoryFixture::new(&manifest);
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert!(output.as_ref().is_ok_and(|value| value.status.success()));
    let receipt = output.as_ref().ok().and_then(parse);
    let log = receipt
        .as_ref()
        .and_then(|value| task(value, "task"))
        .and_then(|value| value["log"].as_str())
        .map(PathBuf::from);
    assert!(
        log.as_ref()
            .and_then(|path| fs::metadata(path).ok())
            .is_some_and(|metadata| metadata.len() > 4 * 1024 * 1024)
    );
}

#[cfg(unix)]
#[test]
fn timeout_allows_graceful_process_group_exit_before_forcing() {
    let manifest = manifest("graceful", "500ms", Path::new("unused"));
    let fixture = RepositoryFixture::new(&manifest);
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(1)
    );
    let receipt = output.as_ref().ok().and_then(parse);
    let result = receipt.as_ref().and_then(|value| task(value, "task"));
    assert_eq!(
        result.and_then(|value| value["termination"]["reason"].as_str()),
        Some("timeout")
    );
    assert_eq!(
        result.and_then(|value| value["termination"]["forced"].as_bool()),
        Some(false)
    );
}

#[test]
fn timeout_force_kills_the_complete_process_tree() {
    let fixture = RepositoryFixture::new(
        "schema = 1\n[execution]\nrepository_state = \"ignore\"\n[tasks.task]\nrun = [\"git\", \"--version\"]\n",
    );
    let heartbeat = fixture.cache().join("timeout-heartbeat");
    let manifest = manifest("orphan-parent", "1500ms", &heartbeat);
    assert!(fixture.write_repository_file("zcheck.toml", &manifest));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(1)
    );
    let receipt = output.as_ref().ok().and_then(parse);
    let result = receipt.as_ref().and_then(|value| task(value, "task"));
    assert_eq!(
        result.and_then(|value| value["termination"]["reason"].as_str()),
        Some("timeout")
    );
    assert_eq!(
        result.and_then(|value| value["termination"]["forced"].as_bool()),
        Some(true)
    );
    assert!(wait_until(Duration::from_secs(1), || heartbeat.is_file()));
    let before = fs::read_to_string(&heartbeat).ok();
    thread::sleep(Duration::from_millis(150));
    assert_eq!(before, fs::read_to_string(&heartbeat).ok());
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_the_tree_and_every_unscheduled_task_with_exit_130() {
    let helper = fixture_binary();
    let fixture = RepositoryFixture::new(
        "schema = 1\n[execution]\nrepository_state = \"ignore\"\n[tasks.task]\nrun = [\"git\", \"--version\"]\n",
    );
    let first_heartbeat = fixture.cache().join("interrupt-heartbeat-one");
    let second_heartbeat = fixture.cache().join("interrupt-heartbeat-two");
    let later = fixture.cache().join("later-marker");
    assert!(fixture.write_repository_file(
        "zcheck.toml",
        &interrupt_manifest(&helper, &first_heartbeat, &second_heartbeat, &later)
    ));
    let mut command = fixture.command(binary());
    command
        .args(["run", "--format", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn();
    assert!(child.is_ok());
    let Some(child) = child.ok() else {
        return;
    };
    assert!(wait_until(Duration::from_secs(5), || {
        first_heartbeat.is_file() && second_heartbeat.is_file()
    }));
    assert!(child.signal(Signal::SIGINT).is_ok());
    let output = child.wait_with_output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(130)
    );
    let receipt = output.as_ref().ok().and_then(parse);
    assert!(
        receipt
            .as_ref()
            .and_then(|value| value["receipt"].as_str())
            .is_some_and(|path| Path::new(path).is_file())
    );
    for name in ["running-one", "running-two", "later", "check"] {
        assert_eq!(
            receipt
                .as_ref()
                .and_then(|value| task(value, name))
                .and_then(|value| value["status"].as_str()),
            Some("cancelled")
        );
    }
    for name in ["running-one", "running-two"] {
        assert_eq!(
            receipt
                .as_ref()
                .and_then(|value| task(value, name))
                .and_then(|value| value["termination"]["reason"].as_str()),
            Some("interrupted")
        );
    }
    assert!(!later.exists());
    let before = [
        fs::read_to_string(&first_heartbeat).ok(),
        fs::read_to_string(&second_heartbeat).ok(),
    ];
    thread::sleep(Duration::from_millis(150));
    assert_eq!(
        before,
        [
            fs::read_to_string(&first_heartbeat).ok(),
            fs::read_to_string(&second_heartbeat).ok(),
        ]
    );
}

#[cfg(unix)]
fn interrupt_manifest(
    helper: &Path,
    first_heartbeat: &Path,
    second_heartbeat: &Path,
    later: &Path,
) -> String {
    let helper = quoted(helper);
    let first_heartbeat = quoted(first_heartbeat);
    let second_heartbeat = quoted(second_heartbeat);
    let later = quoted(later);
    let test_name = fixture::TEST_NAME;
    format!(
        r#"schema = 1
default = "check"
[execution]
jobs = 2
repository_state = "ignore"
[tasks.running-one]
run = [{helper}, "--exact", {test_name:?}, "--nocapture"]
timeout = "20s"
env = {{ {ROLE} = "tree-parent", {HEARTBEAT} = {first_heartbeat} }}
[tasks.running-two]
run = [{helper}, "--exact", {test_name:?}, "--nocapture"]
timeout = "20s"
env = {{ {ROLE} = "tree-parent", {HEARTBEAT} = {second_heartbeat} }}
[tasks.later]
run = [{helper}, "--exact", {test_name:?}, "--nocapture"]
env = {{ {ROLE} = "later", {LATER_MARKER} = {later} }}
[tasks.check]
needs = ["running-one", "running-two", "later"]
"#
    )
}
