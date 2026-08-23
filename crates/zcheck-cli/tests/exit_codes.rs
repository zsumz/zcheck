//! End-to-end compatibility tests for zcheck process exit classification.

use zcheck_testkit::RepositoryFixture;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_zcheck")
}

fn command_manifest(tool: Option<&str>) -> String {
    let tool = tool.map_or_else(String::new, |name| format!("tools = [{name:?}]\n"));
    format!(
        "schema = 1\ndefault = \"check\"\n[execution]\nrepository_state = \"ignore\"\n[tasks.check]\nrun = [{:?}, \"--version\"]\n{tool}",
        binary()
    )
}

fn status(fixture: &RepositoryFixture, arguments: &[&str]) -> Option<i32> {
    fixture
        .command(binary())
        .args(arguments)
        .output()
        .ok()
        .and_then(|output| output.status.code())
}

#[test]
fn published_exit_classes_are_stable_end_to_end() {
    let passed = RepositoryFixture::new(&command_manifest(None));
    assert_eq!(status(&passed, &["--version"]), Some(0));

    let qualification = RepositoryFixture::new(&command_manifest(Some("missing-zcheck-tool")));
    assert_eq!(status(&qualification, &["run"]), Some(1));

    let configuration = RepositoryFixture::new("schema = 2\n[tasks.check]\nrun = [\"git\"]\n");
    assert_eq!(status(&configuration, &["validate"]), Some(2));

    let internal = RepositoryFixture::new(&command_manifest(None));
    let output = internal
        .command(binary())
        .env(
            "ZCHECK_CACHE_DIR",
            internal.repository().join("zcheck.toml"),
        )
        .output()
        .ok();
    assert_eq!(
        output.as_ref().and_then(|value| value.status.code()),
        Some(3)
    );
}
