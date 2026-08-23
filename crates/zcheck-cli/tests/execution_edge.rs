//! Edge contracts for platform projection and preflight.

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
fn skipped_branches_do_not_require_their_unplanned_dependencies() {
    let command = quoted(binary());
    let excluded = if cfg!(windows) { "linux" } else { "windows" };
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.private]
run = ["tool-only-used-by-skipped-branch"]
[tasks.branch]
platforms = ["{excluded}"]
needs = ["private"]
[tasks.independent]
run = [{command}, "--version"]
[tasks.check]
needs = ["branch", "independent"]
"#
    ));
    assert!(fixture.repository().is_dir());
    assert!(fixture.cache().is_dir());
    let output = fixture
        .command(binary())
        .args(["run", "check", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    assert_eq!(
        report
            .as_ref()
            .and_then(|value| task(value, "branch"))
            .and_then(|task| task["status"].as_str()),
        Some("skipped")
    );
    assert_eq!(
        report
            .as_ref()
            .and_then(|value| task(value, "check"))
            .and_then(|task| task["status"].as_str()),
        Some("passed")
    );
}

#[test]
fn task_path_overrides_govern_tool_preflight() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
run = [{command}, "--version"]
tools = ["git"]
env = {{ PATH = "" }}
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
        report
            .as_ref()
            .and_then(|value| task(value, "check"))
            .and_then(|task| task["status"].as_str()),
        Some("blocked")
    );
}

#[test]
fn multiple_roots_share_one_dependency_per_invocation() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
[execution]
repository_state = "ignore"
[tasks.shared]
run = [{command}, "--version"]
[tasks.first]
needs = ["shared"]
run = [{command}, "--version"]
[tasks.second]
needs = ["shared"]
run = [{command}, "--version"]
"#
    ));
    let output = fixture
        .command(binary())
        .args(["run", "first", "second", "--format", "json"])
        .output()
        .ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
    let report = output
        .as_ref()
        .and_then(|value| serde_json::from_slice::<serde_json::Value>(&value.stdout).ok());
    assert_eq!(
        report
            .as_ref()
            .and_then(|value| value["tasks"].as_array())
            .map(|tasks| tasks.iter().filter(|task| task["name"] == "shared").count()),
        Some(1)
    );
    assert_eq!(
        report
            .as_ref()
            .and_then(|value| value["selection"].as_array())
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn runner_storage_failures_use_the_internal_error_exit() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
run = [{command}, "--version"]
"#
    ));
    let output = fixture
        .command(binary())
        .env("ZCHECK_CACHE_DIR", fixture.repository().join("zcheck.toml"))
        .output()
        .ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(3)
    );
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stderr).contains("cannot create log directory")
    }));
}

#[cfg(unix)]
#[test]
fn working_directory_symlinks_may_not_escape_the_repository() {
    let command = quoted(binary());
    let fixture = RepositoryFixture::new(&format!(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
cwd = "outside"
run = [{command}, "--version"]
"#
    ));
    assert!(fixture.symlink_into_repository(fixture.cache(), "outside"));
    let output = fixture.command(binary()).output().ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(2)
    );
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stderr).contains("cwd `outside` resolves outside")
    }));
}

#[cfg(unix)]
#[test]
fn repository_executable_symlinks_may_not_escape_the_repository() {
    let fixture = RepositoryFixture::new(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
run = ["./outside-tool", "--version"]
"#,
    );
    let external = fixture.copy_to_cache(binary(), "external-tool");
    assert!(
        external
            .as_ref()
            .is_some_and(|path| { fixture.symlink_into_repository(path, "outside-tool") })
    );
    let output = fixture.command(binary()).output().ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(2)
    );
    assert!(output.as_ref().is_some_and(|value| {
        String::from_utf8_lossy(&value.stderr)
            .contains("repository-local executable `./outside-tool` resolves outside")
    }));
}

#[cfg(unix)]
#[test]
fn relative_path_entries_may_not_escape_through_symlinks() {
    let fixture = RepositoryFixture::new(
        r#"schema = 1
default = "check"
[execution]
repository_state = "ignore"
[tasks.check]
run = ["tool", "--version"]
env = { PATH = "scripts" }
"#,
    );
    assert!(fixture.create_repository_directory("scripts"));
    let external = fixture.copy_to_cache(binary(), "external-tool");
    assert!(
        external
            .as_ref()
            .is_some_and(|path| { fixture.symlink_into_repository(path, "scripts/tool") })
    );
    let output = fixture.command(binary()).output().ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(2)
    );
    assert!(output.as_ref().is_some_and(|value| {
        let error = String::from_utf8_lossy(&value.stderr);
        error.contains("executable `tool` found through relative PATH entry `scripts`")
            && error.contains("resolves outside the repository root")
    }));
}

#[test]
fn absolute_path_entries_may_resolve_external_tools() {
    let fixture = RepositoryFixture::new(
        "schema = 1\ndefault = \"check\"\n[tasks.check]\nrun = [\"placeholder\"]\n",
    );
    let tool = if cfg!(windows) {
        "external-tool.exe"
    } else {
        "external-tool"
    };
    assert!(fixture.copy_to_cache(binary(), tool).is_some());
    let path = fixture.cache().to_string_lossy().into_owned();
    let manifest = format!(
        "schema = 1\ndefault = \"check\"\n[execution]\nrepository_state = \"ignore\"\n[tasks.check]\nrun = [\"external-tool\", \"--version\"]\nenv = {{ PATH = {path:?} }}\n"
    );
    assert!(fixture.write_repository_file("zcheck.toml", &manifest));
    let output = fixture.command(binary()).output().ok();
    assert!(output.as_ref().is_some_and(|value| value.status.success()));
}
