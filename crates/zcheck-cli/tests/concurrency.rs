//! Black-box contracts for bounded deterministic DAG concurrency.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use zcheck_testkit::RepositoryFixture;

#[path = "concurrency/fixture.rs"]
mod fixture;

const ROLE: &str = "ZCHECK_CONCURRENCY_ROLE";
const EVENTS: &str = "ZCHECK_CONCURRENCY_EVENTS";
const TASK: &str = "ZCHECK_CONCURRENCY_TASK";
const DELAY: &str = "ZCHECK_CONCURRENCY_DELAY_MS";
const MARKER: &str = "ZCHECK_CONCURRENCY_MARKER";

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

fn executable(id: &str, role: &str, delay: u64, events: &Path) -> String {
    let helper = quoted(fixture_binary());
    let test_name = fixture::TEST_NAME;
    let events = quoted(events);
    format!(
        "run = [{helper}, \"--exact\", {test_name:?}, \"--nocapture\"]\nenv = {{ {ROLE} = {role:?}, {TASK} = {id:?}, {DELAY} = \"{delay}\", {EVENTS} = {events}, RUST_BACKTRACE = \"0\" }}\n"
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

fn events(path: &Path) -> Vec<(String, String)> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(event, task)| (event.to_owned(), task.to_owned()))
        .collect()
}

fn maximum_active(events: &[(String, String)], selected: &[&str]) -> usize {
    let selected = selected.iter().copied().collect::<BTreeSet<_>>();
    let mut active = BTreeSet::new();
    let mut maximum = 0;
    for (event, task) in events {
        if !selected.contains(task.as_str()) {
            continue;
        }
        if event == "start" {
            active.insert(task.clone());
            maximum = maximum.max(active.len());
        } else if event == "end" {
            active.remove(task);
        }
    }
    maximum
}

#[test]
fn jobs_override_bounds_parallelism_and_receipts_stay_in_plan_order() {
    let fixture = RepositoryFixture::new("schema = 1\n[tasks.check]\nrun = [\"git\"]\n");
    let event_path = fixture.cache().join("jobs-events");
    let manifest = format!(
        "schema = 1\ndefault = \"check\"\n[execution]\njobs = 1\nrepository_state = \"ignore\"\n[tasks.a]\n{}[tasks.b]\n{}[tasks.c]\n{}[tasks.d]\n{}[tasks.check]\nneeds = [\"a\", \"b\", \"c\", \"d\"]\n",
        executable("a", "work", 200, &event_path),
        executable("b", "work", 200, &event_path),
        executable("c", "work", 200, &event_path),
        executable("d", "work", 200, &event_path),
    );
    assert!(fixture.write_repository_file("zcheck.toml", &manifest));
    let output = fixture
        .command(binary())
        .args(["run", "--jobs", "2", "--format", "json"])
        .output();
    assert!(output.as_ref().is_ok_and(|value| value.status.success()));
    assert!(output.as_ref().is_ok_and(|value| value.stderr.is_empty()));
    let observed = events(&event_path);
    assert_eq!(maximum_active(&observed, &["a", "b", "c", "d"]), 2);
    let receipt = output.as_ref().ok().and_then(parse);
    let names = receipt.as_ref().and_then(|value| {
        value["tasks"].as_array().map(|tasks| {
            tasks
                .iter()
                .filter_map(|task| task["name"].as_str())
                .collect::<Vec<_>>()
        })
    });
    assert_eq!(names, Some(vec!["a", "b", "c", "d", "check"]));
}

#[test]
fn resources_are_exclusive_without_blocking_unrelated_work() {
    let fixture = RepositoryFixture::new("schema = 1\n[tasks.check]\nrun = [\"git\"]\n");
    let event_path = fixture.cache().join("resource-events");
    let manifest = format!(
        "schema = 1\ndefault = \"check\"\n[execution]\njobs = 3\nrepository_state = \"ignore\"\n[tasks.a]\n{}resources = [\"shared\"]\n[tasks.b]\n{}resources = [\"shared\"]\n[tasks.c]\n{}[tasks.check]\nneeds = [\"a\", \"b\", \"c\"]\n",
        executable("a", "work", 250, &event_path),
        executable("b", "work", 250, &event_path),
        executable("c", "work", 700, &event_path),
    );
    assert!(fixture.write_repository_file("zcheck.toml", &manifest));
    let output = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert!(output.as_ref().is_ok_and(|value| value.status.success()));
    let observed = events(&event_path);
    assert_eq!(maximum_active(&observed, &["a", "b"]), 1);
    assert_eq!(maximum_active(&observed, &["a", "b", "c"]), 2);
    let shared_starts = observed
        .iter()
        .filter(|(event, task)| event == "start" && matches!(task.as_str(), "a" | "b"))
        .map(|(_, task)| task.as_str())
        .collect::<Vec<_>>();
    assert_eq!(shared_starts, ["a", "b"]);
}

#[test]
fn executable_descendants_wait_for_every_dependency() {
    let fixture = RepositoryFixture::new("schema = 1\n[tasks.check]\nrun = [\"git\"]\n");
    let event_path = fixture.cache().join("dependency-events");
    let manifest = format!(
        "schema = 1\ndefault = \"check\"\n[execution]\njobs = 3\nrepository_state = \"ignore\"\n[tasks.a]\n{}[tasks.b]\n{}[tasks.child]\n{}needs = [\"a\", \"b\"]\n[tasks.check]\nneeds = [\"child\"]\n",
        executable("a", "work", 150, &event_path),
        executable("b", "work", 300, &event_path),
        executable("child", "work", 10, &event_path),
    );
    assert!(fixture.write_repository_file("zcheck.toml", &manifest));
    let output = fixture.command(binary()).arg("run").output();
    assert!(output.as_ref().is_ok_and(|value| value.status.success()));
    let observed = events(&event_path);
    let child_start = observed
        .iter()
        .position(|event| event == &("start".to_owned(), "child".to_owned()));
    for dependency in ["a", "b"] {
        let end = observed
            .iter()
            .position(|event| event == &("end".to_owned(), dependency.to_owned()));
        assert!(
            end.zip(child_start)
                .is_some_and(|(left, right)| left < right)
        );
    }
}

#[test]
fn fail_fast_stops_new_work_but_allows_running_tasks_to_finish() {
    let fixture = RepositoryFixture::new("schema = 1\n[tasks.check]\nrun = [\"git\"]\n");
    let event_path = fixture.cache().join("fail-fast-events");
    let marker = fixture.cache().join("later-marker");
    let later = executable("later", "work", 10, &event_path)
        .replace(" }\n", &format!(", {MARKER} = {} }}\n", quoted(&marker)));
    let manifest = format!(
        "schema = 1\ndefault = \"check\"\n[execution]\njobs = 2\nrepository_state = \"ignore\"\n[tasks.fail]\n{}[tasks.running]\n{}[tasks.later]\n{later}[tasks.check]\nneeds = [\"fail\", \"running\", \"later\"]\n",
        executable("fail", "fail", 100, &event_path),
        executable("running", "work", 400, &event_path),
    );
    assert!(fixture.write_repository_file("zcheck.toml", &manifest));
    let output = fixture
        .command(binary())
        .args(["run", "--fail-fast", "--format", "json"])
        .output();
    assert_eq!(
        output.as_ref().ok().and_then(|value| value.status.code()),
        Some(1)
    );
    let receipt = output.as_ref().ok().and_then(parse);
    for (name, status) in [
        ("fail", "failed"),
        ("running", "passed"),
        ("later", "blocked"),
        ("check", "failed"),
    ] {
        assert_eq!(
            receipt
                .as_ref()
                .and_then(|value| task(value, name))
                .and_then(|task| task["status"].as_str()),
            Some(status)
        );
    }
    assert!(!marker.exists());
    assert!(
        receipt
            .as_ref()
            .and_then(|value| task(value, "later"))
            .and_then(|task| task["reason"].as_str())
            .is_some_and(|reason| reason.contains("fail-fast stopped scheduling"))
    );

    let continued = fixture
        .command(binary())
        .args(["run", "--format", "json"])
        .output();
    assert_eq!(
        continued
            .as_ref()
            .ok()
            .and_then(|value| value.status.code()),
        Some(1)
    );
    let continued = continued.as_ref().ok().and_then(parse);
    assert_eq!(
        continued
            .as_ref()
            .and_then(|value| task(value, "later"))
            .and_then(|task| task["status"].as_str()),
        Some("passed")
    );
    assert!(marker.is_file());
}

#[test]
fn human_progress_reports_starts_and_completions_without_stream_interleaving() {
    let fixture = RepositoryFixture::new("schema = 1\n[tasks.check]\nrun = [\"git\"]\n");
    let event_path = fixture.cache().join("reporter-events");
    let manifest = format!(
        "schema = 1\ndefault = \"check\"\n[execution]\njobs = 2\nrepository_state = \"ignore\"\n[tasks.a]\n{}[tasks.b]\n{}[tasks.check]\nneeds = [\"a\", \"b\"]\n",
        executable("a", "work", 50, &event_path),
        executable("b", "work", 50, &event_path),
    );
    assert!(fixture.write_repository_file("zcheck.toml", &manifest));
    let output = fixture.command(binary()).arg("run").output();
    assert!(output.as_ref().is_ok_and(|value| value.status.success()));
    let stderr = output.as_ref().map_or_else(
        |_| String::new(),
        |value| String::from_utf8_lossy(&value.stderr).into_owned(),
    );
    for expected in ["START  a", "START  b", "PASS   a", "PASS   b"] {
        assert!(
            stderr.contains(expected),
            "missing progress line {expected}"
        );
    }
    let lines = stderr.lines().collect::<Vec<_>>();
    let position = |needle: &str| lines.iter().position(|line| line.starts_with(needle));
    assert!(
        position("START  a")
            .zip(position("START  b"))
            .is_some_and(|(a, b)| a < b),
        "ready tasks must be reported in deterministic plan order"
    );
    for task in ["a", "b"] {
        assert!(
            position(&format!("START  {task}"))
                .zip(position(&format!("PASS   {task}")))
                .is_some_and(|(start, finish)| start < finish),
            "task completion must follow its start"
        );
    }
    assert!(stderr.lines().all(|line| {
        line.starts_with("START")
            || line.starts_with("PASS")
            || line.starts_with("FAIL")
            || line.starts_with("BLOCK")
            || line.starts_with("SKIP")
            || line.starts_with("CANCEL")
    }));
}
