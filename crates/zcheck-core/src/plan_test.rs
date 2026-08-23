//! Tests for deterministic shared dependency and platform planning.

use crate::{Manifest, Platform, PlatformApplicability, TaskName};

fn names(plan: &crate::Plan) -> Vec<&str> {
    plan.tasks()
        .iter()
        .map(|task| task.name().as_str())
        .collect()
}

#[test]
fn resolves_shared_dependencies_once_in_dependency_order() {
    let source = r#"
schema = 1

[tasks.provenance]
run = ["scripts/provenance"]

[tasks.lint]
needs = ["provenance"]
run = ["cargo", "clippy"]

[tasks.test]
needs = ["provenance"]
run = ["cargo", "test"]

[tasks.check]
needs = ["lint", "test"]
"#;
    let manifest = Manifest::parse(source);
    let selection = "check".parse::<TaskName>().ok();
    let plan = manifest
        .ok()
        .zip(selection)
        .and_then(|(value, name)| value.plan(&[name], Platform::Linux).ok());
    assert_eq!(
        plan.as_ref().map(names),
        Some(vec!["provenance", "lint", "test", "check"])
    );
    assert!(
        plan.as_ref()
            .and_then(|value| value.tasks().last())
            .is_some_and(|task| task.timeout().is_none())
    );
}

#[test]
fn retains_the_skipped_platform_branch_without_its_private_dependencies() {
    let source = r#"
schema = 1

[tasks.windows-setup]
run = ["pwsh", "-File", "scripts/setup.ps1"]

[tasks.windows]
needs = ["windows-setup"]
run = ["pwsh", "-File", "scripts/smoke.ps1"]
platforms = ["windows"]

[tasks.unix]
run = ["scripts/smoke"]
platforms = ["linux", "macos"]

[tasks.smoke]
needs = ["unix", "windows"]
"#;
    let manifest = Manifest::parse(source);
    let selection = "smoke".parse::<TaskName>().ok();
    let plan = manifest
        .ok()
        .zip(selection)
        .and_then(|(value, name)| value.plan(&[name], Platform::Macos).ok());
    assert_eq!(
        plan.as_ref().map(names),
        Some(vec!["unix", "windows", "smoke"])
    );
    assert_eq!(
        plan.as_ref()
            .and_then(|value| value.tasks().get(1))
            .map(super::PlannedTask::applicability),
        Some(PlatformApplicability::Skipped)
    );
}

#[test]
fn serializes_a_versioned_plan_without_environment_values() {
    let source = r#"
schema = 1

[tasks.check]
run = ["cargo", "test"]
env = { TOKEN = "secret" }
"#;
    let manifest = Manifest::parse(source);
    let selection = "check".parse::<TaskName>().ok();
    let plan = manifest
        .ok()
        .zip(selection)
        .and_then(|(value, name)| value.plan(&[name], Platform::Linux).ok());
    let json = plan
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok());
    assert!(
        json.as_ref()
            .is_some_and(|value| value.contains("\"schema\":1"))
    );
    assert!(
        json.as_ref()
            .is_some_and(|value| value.contains("\"TOKEN\""))
    );
    assert!(json.as_ref().is_some_and(|value| !value.contains("secret")));
}

#[test]
fn projects_default_and_task_timeout_precedence_only_onto_processes() {
    let source = r#"
schema = 1
[execution]
default_timeout = "20m"

[tasks.defaulted]
run = ["cargo", "test"]

[tasks.overridden]
run = ["cargo", "doc"]
timeout = "30s"

[tasks.check]
needs = ["defaulted", "overridden"]
"#;
    let manifest = Manifest::parse(source);
    let selection = "check".parse::<TaskName>().ok();
    let plan = manifest
        .ok()
        .zip(selection)
        .and_then(|(value, name)| value.plan(&[name], Platform::Linux).ok());
    let timeouts = plan.as_ref().map(|value| {
        value
            .tasks()
            .iter()
            .map(|task| task.timeout().map(crate::DurationSpec::as_str))
            .collect::<Vec<_>>()
    });
    assert_eq!(timeouts, Some(vec![Some("20m"), Some("30s"), None]));
}
