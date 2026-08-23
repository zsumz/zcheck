//! Golden compatibility tests for the published schema-1 contracts.

use crate::{Manifest, Plan, Platform, Receipt, RunStatus, TaskName, TaskStatus};

const MANIFEST_CONTRACT: &str = include_str!("../contracts/schema-1/manifest.toml");
const PLAN_CONTRACT: &str = include_str!("../contracts/schema-1/plan-linux.json");
const RECEIPT_CONTRACT: &str = include_str!("../contracts/schema-1/receipt.json");

#[test]
fn manifest_and_linux_plan_match_the_schema_one_golden_contract() {
    let actual = Manifest::parse(MANIFEST_CONTRACT)
        .map_err(|error| error.to_string())
        .and_then(|manifest| {
            assert_eq!(manifest.schema(), Manifest::SCHEMA);
            let check = "check"
                .parse::<TaskName>()
                .map_err(|error| error.to_string())?;
            manifest
                .plan(&[check], Platform::Linux)
                .map_err(|error| error.to_string())
        })
        .and_then(|plan| {
            assert_eq!(plan.schema(), Plan::SCHEMA);
            plan.to_json_pretty().map_err(|error| error.to_string())
        });
    assert_eq!(actual.as_deref(), Ok(PLAN_CONTRACT.trim_end()));
}

#[test]
fn receipt_round_trips_the_schema_one_golden_contract() {
    let parsed = Receipt::parse_json(RECEIPT_CONTRACT).map_err(|error| error.to_string());
    assert_eq!(
        parsed.as_ref().map(|value| value.schema),
        Ok(Receipt::SCHEMA)
    );
    assert_eq!(
        parsed.as_ref().map(|value| value.status),
        Ok(RunStatus::Cancelled)
    );
    assert_eq!(
        parsed.as_ref().map(|value| {
            value
                .tasks
                .iter()
                .map(|task| task.status)
                .collect::<Vec<_>>()
        }),
        Ok(vec![
            TaskStatus::Passed,
            TaskStatus::Failed,
            TaskStatus::Blocked,
            TaskStatus::Skipped,
            TaskStatus::Cancelled,
        ])
    );
    let actual = parsed.and_then(|value| value.to_json_pretty().map_err(|error| error.to_string()));
    assert_eq!(actual.as_deref(), Ok(RECEIPT_CONTRACT.trim_end()));
}

#[test]
fn receipt_parser_rejects_unknown_versions_and_fields() {
    let unsupported = RECEIPT_CONTRACT.replacen("\"schema\": 1", "\"schema\": 2", 1);
    assert_eq!(
        Receipt::parse_json(&unsupported).map_err(|error| error.to_string()),
        Err("unsupported receipt schema 2; this zcheck supports schema 1".to_owned())
    );
    let unknown = RECEIPT_CONTRACT.replacen(
        "\"schema\": 1,",
        "\"schema\": 1,\n  \"unexpected\": true,",
        1,
    );
    assert!(
        Receipt::parse_json(&unknown)
            .map_err(|error| error.to_string())
            .is_err_and(|error| error.contains("unknown field"))
    );
}
