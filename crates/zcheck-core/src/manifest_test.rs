//! Tests for strict manifest decoding.

use super::Manifest;

const VALID: &str = r#"
schema = 1
default = "check"

[execution]
jobs = 2
default_timeout = "20m"
repository_state = "preserve"

[tasks.test]
description = "Run tests."
run = ["cargo", "test"]

[tasks.check]
needs = ["test"]
"#;

#[test]
fn decodes_a_complete_manifest() {
    let manifest = Manifest::parse(VALID);
    assert_eq!(manifest.as_ref().map(Manifest::schema), Ok(1));
    assert_eq!(
        manifest.as_ref().map(|value| value.execution().jobs()),
        Ok(2)
    );
    assert_eq!(
        manifest
            .as_ref()
            .ok()
            .and_then(Manifest::default_task)
            .map(ToString::to_string),
        Some("check".to_owned())
    );
}

#[test]
fn rejects_unknown_fields_at_every_schema_level() {
    let root = VALID.replace("schema = 1", "schema = 1\nschemma = 1");
    let execution = VALID.replace("jobs = 2", "jobs = 2\njob = 4");
    let task = VALID.replace("run = [\"cargo\", \"test\"]", "rnu = [\"cargo\"]");
    for source in [&root, &execution, &task] {
        let error = Manifest::parse(source).map_err(|value| value.to_string());
        assert!(
            error.is_err_and(|message| message.contains("unknown field")),
            "unknown field was not reported"
        );
    }
}

#[test]
fn rejects_unknown_schema_versions() {
    let error = Manifest::parse(&VALID.replace("schema = 1", "schema = 2"))
        .map_err(|value| value.to_string());
    assert_eq!(
        error,
        Err("unsupported manifest schema 2; this zcheck supports schema 1".to_owned())
    );
}
