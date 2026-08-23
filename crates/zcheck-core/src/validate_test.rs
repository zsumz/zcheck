//! Tests for fail-closed semantic and graph validation.

use crate::Manifest;

fn manifest(tasks: &str) -> String {
    format!("schema = 1\ndefault = \"check\"\n{tasks}")
}

#[test]
fn reports_a_complete_cycle() {
    let source = manifest(
        r#"
[tasks.check]
needs = ["package"]

[tasks.package]
needs = ["check"]
"#,
    );
    let error = Manifest::parse(&source).map_err(|value| value.to_string());
    assert_eq!(
        error,
        Err("invalid task graph:\ncheck -> package -> check".to_owned())
    );
}

#[test]
fn rejects_unknown_dependencies_and_empty_tasks() {
    let unknown = manifest("[tasks.check]\nneeds = [\"missing\"]\n");
    assert!(
        Manifest::parse(&unknown)
            .map_err(|value| value.to_string())
            .is_err_and(|message| message.contains("unknown task `missing`"))
    );
    let empty = manifest("[tasks.check]\ndescription = \"Nothing.\"\n");
    assert!(
        Manifest::parse(&empty)
            .map_err(|value| value.to_string())
            .is_err_and(|message| message.contains("must define run, needs, or both"))
    );
}

#[test]
fn rejects_paths_that_may_escape_on_any_supported_platform() {
    for field in [
        "cwd = \"../outside\"",
        "inputs = [\"..\\\\outside\"]",
        "run = [\"../outside\"]",
    ] {
        let run = if field.starts_with("run") {
            String::new()
        } else {
            "run = [\"cargo\"]\n".to_owned()
        };
        let source = manifest(&format!("[tasks.check]\n{run}{field}\n"));
        assert!(
            Manifest::parse(&source)
                .map_err(|value| value.to_string())
                .is_err_and(|message| message.contains("repository root")),
            "accepted {field}"
        );
    }
}

#[test]
fn rejects_duplicate_graph_and_scheduling_metadata() {
    let source = manifest(
        r#"
[tasks.leaf]
run = ["cargo", "test"]

[tasks.check]
needs = ["leaf", "leaf"]
"#,
    );
    assert!(
        Manifest::parse(&source)
            .map_err(|value| value.to_string())
            .is_err_and(|message| message.contains("duplicate dependency `leaf`"))
    );
}

#[test]
fn rejects_process_only_fields_on_aggregate_tasks() {
    for field in [
        "cwd = \".\"",
        "env = { MARKER = \"present\" }",
        "tools = [\"cargo\"]",
        "timeout = \"1s\"",
        "resources = [\"cargo-target\"]",
    ] {
        let source = manifest(&format!(
            "[tasks.leaf]\nrun = [\"cargo\"]\n[tasks.check]\nneeds = [\"leaf\"]\n{field}\n"
        ));
        assert!(
            Manifest::parse(&source)
                .map_err(|value| value.to_string())
                .is_err_and(|message| message.contains("process field")),
            "accepted {field}"
        );
    }
}

#[test]
fn rejects_unrepresentable_process_values_and_escaping_tool_paths() {
    for field in [
        "run = [\"cargo\", \"a\\u0000b\"]",
        "run = [\"cargo\"]\nenv = { MARKER = \"a\\u0000b\" }",
        "run = [\"cargo\"]\ntools = [\"..\\\\outside\"]",
    ] {
        let source = manifest(&format!("[tasks.check]\n{field}\n"));
        assert!(Manifest::parse(&source).is_err(), "accepted {field}");
    }
}

#[test]
fn accepts_platform_specific_absolute_executable_paths() {
    for executable in [r"/usr/bin/tool", r"C:\Tools\tool.exe"] {
        let source = manifest(&format!("[tasks.check]\nrun = [{executable:?}]\n"));
        assert!(Manifest::parse(&source).is_ok(), "rejected {executable}");
    }
}
