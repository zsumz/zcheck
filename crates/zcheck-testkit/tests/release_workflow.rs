//! Release workflow structure is explicit, complete, and protected.

use std::{fs, path::PathBuf};

#[test]
fn release_requires_protected_tagged_source_before_repository_execution() {
    let workflow = release_workflow();
    let ancestry = workflow.find("git merge-base --is-ancestor");
    let local_action = workflow.find("uses: ./.github/actions/setup-rust");

    assert!(workflow.contains("tags: [\"v*\"]"));
    assert!(!workflow.contains("pull_request:"));
    assert!(!workflow.contains("workflow_dispatch:"));
    assert_eq!(workflow.matches("environment: release").count(), 2);
    assert!(
        ancestry
            .zip(local_action)
            .is_some_and(|(left, right)| left < right)
    );
    assert!(workflow.contains("test \"$version\" = \"$manifest_version\""));
    assert!(workflow.contains("Run complete qualification gate offline"));
    assert!(workflow.contains("cargo run --locked --offline -p zcheck -- run check"));
    assert!(workflow.contains("Upload tag qualification evidence"));
    assert!(!workflow.contains("--accept-grants"));
    assert!(!workflow.contains("--allow-grants"));
}

#[test]
fn release_covers_the_exact_native_target_and_archive_set() {
    let workflow = release_workflow();
    let targets = [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ];
    for target in targets {
        assert!(workflow.contains(target), "missing release target {target}");
    }

    assert!(workflow.contains("runner: macos-15-intel"));
    assert!(workflow.contains("runner: macos-15"));
    assert!(workflow.contains("runner: windows-2025"));
    assert!(workflow.contains("runner: ubuntu-24.04-arm"));
    assert!(workflow.contains("binary=\"$binary.exe\""));
    assert!(workflow.contains(".tar.gz"));
    assert!(workflow.contains(".zip"));
    assert!(workflow.contains("test \"$(\"$binary\" --version)\""));
    assert!(workflow.contains("package-release.py"));
}

#[test]
fn publishing_waits_for_all_builds_and_clean_linux_runtime_checks() {
    let workflow = release_workflow();
    let publish = section(&workflow, "  publish:", "__end_of_workflow__");

    assert!(workflow.contains("needs: [qualify, build]"));
    assert!(publish.contains("needs: [qualify, build, linux-runtime]"));
    assert!(workflow.contains("ubuntu:24.04"));
    assert!(workflow.contains("alpine:3.22"));
    assert!(workflow.contains("Run a qualification graph without a Rust toolchain"));
    assert!(workflow.contains("docker run --rm"));
    assert!(publish.contains("sha256sum --check SHA256SUMS"));
    assert!(publish.contains("actions/attest@"));
    assert!(publish.contains("release-notes.py"));
    assert!(publish.contains("gh release create \"$GITHUB_REF_NAME\""));
    assert!(publish.contains("--verify-tag --draft"));
    assert!(publish.contains("gh release edit \"$GITHUB_REF_NAME\" --draft=false"));
    assert_eq!(workflow.matches("contents: write").count(), 1);
}

#[test]
fn every_external_release_action_uses_an_immutable_full_sha() {
    for line in release_workflow().lines() {
        let Some(reference) = line.trim().strip_prefix("uses: ") else {
            continue;
        };
        let reference = reference.split_whitespace().next();
        assert!(reference.is_some());
        let Some(reference) = reference else {
            continue;
        };
        if reference.starts_with("./") {
            continue;
        }
        let revision = reference.split_once('@').map(|(_, revision)| revision);
        assert_eq!(
            revision.map(str::len),
            Some(40),
            "action is not full-SHA pinned: {line}"
        );
        assert!(
            revision.is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_hexdigit())),
            "action is not hex-SHA pinned: {line}"
        );
    }
}

#[test]
fn readme_prefers_verified_prebuilt_binaries_to_the_cargo_fallback() {
    let readme = fs::read_to_string(repository_root().join("README.md")).unwrap_or_default();
    let binary = readme.find("Download a verified binary");
    let fallback = readme.find("### Cargo fallback");

    assert!(
        binary
            .zip(fallback)
            .is_some_and(|(left, right)| left < right)
    );
    assert!(readme.contains("releases/download/v${ZCHECK_VERSION}"));
    assert!(readme.contains("sha256sum --check"));
    assert!(readme.contains("gh attestation verify"));
}

fn release_workflow() -> String {
    fs::read_to_string(repository_root().join(".github/workflows/release.yml")).unwrap_or_default()
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source.find(start).unwrap_or(source.len());
    let tail = &source[start..];
    let end = tail.find(end).unwrap_or(tail.len());
    &tail[..end]
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map_or_else(PathBuf::new, std::path::Path::to_path_buf)
}
