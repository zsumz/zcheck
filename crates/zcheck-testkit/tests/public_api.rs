//! External-consumer proof for the published zcheck-core analysis surface.

use zcheck_core::{
    DurationSpec, ExecutionPolicy, Manifest, Plan, PlannedTask, PlannedTaskKind, Platform,
    PlatformApplicability, RepositoryStatePolicy, Task, TaskName,
};

const MANIFEST: &str = r#"
schema = 1
default = "check"

[execution]
jobs = 3
default_timeout = "10m"
repository_state = "clean"

[tasks.architecture]
description = "Review architecture policy."
run = ["zrail", "check"]
cwd = "."
env = { ZRAIL_MODE = "reviewed" }
tools = ["zrail"]
timeout = "2m"
platforms = ["linux", "macos"]
resources = ["review-lock"]
inputs = ["zrail.toml", "zrail.lock", "scripts/guardrails"]

[tasks.guardrails]
description = "Run repository guardrails."
run = ["scripts/guardrails"]

[tasks.check]
description = "Qualify the repository."
needs = ["architecture", "guardrails"]
inputs = ["docs/qualification.md"]
"#;

#[test]
fn static_consumers_can_inspect_every_review_relevant_field() {
    let manifest = Manifest::parse(MANIFEST);
    assert_eq!(
        manifest.as_ref().map(Manifest::schema),
        Ok(Manifest::SCHEMA)
    );
    assert_eq!(
        manifest
            .as_ref()
            .ok()
            .and_then(Manifest::default_task)
            .map(TaskName::as_str),
        Some("check")
    );
    assert_eq!(
        manifest
            .as_ref()
            .map(Manifest::tasks)
            .map(std::collections::BTreeMap::len),
        Ok(3)
    );
    let execution = manifest.as_ref().map(Manifest::execution);
    assert_eq!(execution.map(ExecutionPolicy::jobs), Ok(3));
    assert_eq!(
        execution
            .ok()
            .and_then(ExecutionPolicy::default_timeout)
            .map(DurationSpec::as_str),
        Some("10m")
    );
    assert_eq!(
        execution.map(ExecutionPolicy::repository_state),
        Ok(RepositoryStatePolicy::Clean)
    );
    let architecture = "architecture".parse::<TaskName>();
    let task = manifest
        .as_ref()
        .ok()
        .zip(architecture.as_ref().ok())
        .and_then(|(manifest, name)| manifest.task(name));
    assert_eq!(
        task.map(Task::description),
        Some("Review architecture policy.")
    );
    assert_eq!(
        task.and_then(Task::run),
        Some(&["zrail".to_owned(), "check".to_owned()][..])
    );
    assert_eq!(task.map(Task::needs), Some(&[][..]));
    assert_eq!(task.and_then(Task::cwd), Some("."));
    assert_eq!(
        task.map(|value| value.env().keys().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["ZRAIL_MODE"])
    );
    assert_eq!(
        task.and_then(|value| value.env().get("ZRAIL_MODE"))
            .map(String::as_str),
        Some("reviewed")
    );
    assert_eq!(task.map(Task::tools), Some(&["zrail".to_owned()][..]));
    assert_eq!(
        task.and_then(|value| value.timeout())
            .map(DurationSpec::as_str),
        Some("2m")
    );
    assert_eq!(
        task.map(Task::platforms),
        Some(&[Platform::Linux, Platform::Macos][..])
    );
    assert_eq!(
        task.map(Task::resources),
        Some(&["review-lock".to_owned()][..])
    );
    assert_eq!(
        task.map(Task::inputs),
        Some(
            &[
                "zrail.toml".to_owned(),
                "zrail.lock".to_owned(),
                "scripts/guardrails".to_owned()
            ][..]
        )
    );
    let local_programs = manifest.as_ref().map(|value| {
        value
            .tasks()
            .values()
            .filter_map(Task::run)
            .filter_map(|command| command.first())
            .filter(|program| program.starts_with("scripts/"))
            .map(String::as_str)
            .collect::<Vec<_>>()
    });
    assert_eq!(local_programs, Ok(vec!["scripts/guardrails"]));
    let check = "check".parse::<TaskName>();
    assert_eq!(
        manifest
            .as_ref()
            .ok()
            .zip(check.as_ref().ok())
            .and_then(|(value, name)| value.task(name))
            .map(Task::needs)
            .map(|needs| needs.iter().map(TaskName::as_str).collect::<Vec<_>>()),
        Some(vec!["architecture", "guardrails"])
    );
}

#[test]
fn static_consumers_can_resolve_every_supported_platform_without_execution() {
    let manifest = Manifest::parse(MANIFEST);
    let check = "check".parse::<TaskName>();
    assert!(manifest.is_ok());
    assert!(check.is_ok());
    if let (Ok(manifest), Ok(check)) = (manifest, check) {
        for (platform, applicable) in [
            (Platform::Linux, true),
            (Platform::Macos, true),
            (Platform::Windows, false),
        ] {
            let plan = manifest.plan(std::slice::from_ref(&check), platform);
            assert_eq!(plan.as_ref().map(Plan::schema), Ok(Plan::SCHEMA));
            assert_eq!(plan.as_ref().map(Plan::platform), Ok(platform));
            assert_eq!(plan.as_ref().map(Plan::jobs), Ok(3));
            assert_eq!(
                plan.as_ref().map(Plan::selection),
                Ok(std::slice::from_ref(&check))
            );
            assert_eq!(
                plan.as_ref().map(Plan::tasks).map(<[PlannedTask]>::len),
                Ok(3)
            );
            let architecture = plan.as_ref().ok().and_then(|value| value.tasks().first());
            assert_eq!(
                architecture.map(PlannedTask::name).map(TaskName::as_str),
                Some("architecture")
            );
            assert_eq!(
                architecture.map(PlannedTask::description),
                Some("Review architecture policy.")
            );
            assert_eq!(
                architecture.map(PlannedTask::kind),
                Some(PlannedTaskKind::Executable)
            );
            assert_eq!(
                architecture.map(PlannedTask::command),
                Some(Some(&["zrail".to_owned(), "check".to_owned()][..]))
            );
            assert_eq!(architecture.map(PlannedTask::needs), Some(&[][..]));
            assert_eq!(architecture.map(PlannedTask::cwd), Some("."));
            assert_eq!(
                architecture.map(PlannedTask::environment),
                Some(&["ZRAIL_MODE".to_owned()][..])
            );
            assert_eq!(
                architecture.map(PlannedTask::tools),
                Some(&["zrail".to_owned()][..])
            );
            assert_eq!(
                architecture.map(PlannedTask::resources),
                Some(&["review-lock".to_owned()][..])
            );
            assert_eq!(
                architecture.map(PlannedTask::inputs),
                Some(
                    &[
                        "zrail.toml".to_owned(),
                        "zrail.lock".to_owned(),
                        "scripts/guardrails".to_owned()
                    ][..]
                )
            );
            assert_eq!(
                architecture.map(PlannedTask::applicability),
                Some(if applicable {
                    PlatformApplicability::Applicable
                } else {
                    PlatformApplicability::Skipped
                })
            );
            assert_eq!(
                architecture
                    .and_then(PlannedTask::timeout)
                    .map(DurationSpec::as_str),
                Some("2m")
            );
            let aggregate = plan.as_ref().ok().and_then(|value| value.tasks().last());
            assert_eq!(
                aggregate.map(PlannedTask::name).map(TaskName::as_str),
                Some("check")
            );
            assert_eq!(
                aggregate.map(PlannedTask::kind),
                Some(PlannedTaskKind::Aggregate)
            );
            assert_eq!(aggregate.and_then(PlannedTask::command), None);
            assert_eq!(
                aggregate
                    .map(PlannedTask::needs)
                    .map(|needs| { needs.iter().map(TaskName::as_str).collect::<Vec<_>>() }),
                Some(vec!["architecture", "guardrails"])
            );
            assert_eq!(
                aggregate.map(PlannedTask::inputs),
                Some(&["docs/qualification.md".to_owned()][..])
            );
        }
    }
}
