//! Cross-repository design-fixture validation.

use std::fs;
use std::path::{Path, PathBuf};

use zcheck_core::{Manifest, Platform, TaskName};

const REPOSITORIES: [&str; 5] = ["kafkars", "zrail", "zhold", "rafter", "zolt"];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/manifests")
        .join(name)
        .join("zcheck.toml")
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(PathBuf::new, Path::to_path_buf)
}

#[test]
fn all_design_repositories_have_valid_complete_check_plans() {
    for repository in REPOSITORIES {
        let path = fixture(repository);
        let source = fs::read_to_string(&path);
        assert!(source.is_ok(), "cannot read {}", path.display());
        let manifest = source
            .as_deref()
            .map_err(ToString::to_string)
            .and_then(|text| Manifest::parse(text).map_err(|error| error.to_string()));
        assert!(manifest.is_ok(), "{repository}: {manifest:?}");
        let check = "check".parse::<TaskName>();
        for platform in [Platform::Linux, Platform::Macos, Platform::Windows] {
            let plan = manifest.as_ref().map_err(Clone::clone).and_then(|value| {
                value
                    .plan(
                        &[check.clone().map_err(|error| error.to_string())?],
                        platform,
                    )
                    .map_err(|error| error.to_string())
            });
            assert!(plan.is_ok(), "{repository} on {platform}: {plan:?}");
        }
    }
}

#[test]
fn fixtures_use_argument_vectors_instead_of_shell_strings() {
    for repository in REPOSITORIES {
        let path = fixture(repository);
        let source = fs::read_to_string(&path);
        let manifest = source
            .as_deref()
            .map_err(ToString::to_string)
            .and_then(|text| Manifest::parse(text).map_err(|error| error.to_string()));
        assert!(manifest.is_ok(), "{repository}: {manifest:?}");
        if let Ok(manifest) = manifest {
            for (name, task) in manifest.tasks() {
                if let Some(command) = task.run() {
                    assert!(!command.is_empty(), "{repository}:{name} has no executable");
                }
            }
        }
    }
}

#[test]
fn kafkars_complete_check_includes_package_qualification() {
    let source = fs::read_to_string(fixture("kafkars"));
    let manifest = source
        .as_deref()
        .map_err(ToString::to_string)
        .and_then(|text| Manifest::parse(text).map_err(|error| error.to_string()));
    let check = "check".parse::<TaskName>();
    let plan = manifest.as_ref().map_err(Clone::clone).and_then(|value| {
        value
            .plan(
                &[check.map_err(|error| error.to_string())?],
                Platform::Linux,
            )
            .map_err(|error| error.to_string())
    });
    assert!(plan.as_ref().is_ok_and(|plan| {
        plan.tasks()
            .iter()
            .any(|task| task.name().as_str() == "package")
    }));
}

#[test]
fn package_gate_tests_self_contained_publishable_sources() {
    let root = workspace();
    let manifest = fs::read_to_string(root.join("crates/zcheck-cli/Cargo.toml"));
    let gate = fs::read_to_string(root.join("scripts/package-check"));
    assert!(
        manifest
            .as_ref()
            .is_ok_and(|source| { source.contains("exclude = [\"tests/**\"]") })
    );
    assert!(gate.as_ref().is_ok_and(|source| {
        source.contains("contains workspace-only integration tests")
            && source.matches("cargo test --manifest-path").count() == 2
            && source.contains("contracts/schema-1/manifest.toml")
            && source.contains("contracts/schema-1/plan-linux.json")
            && source.contains("contracts/schema-1/receipt.json")
    }));
}
