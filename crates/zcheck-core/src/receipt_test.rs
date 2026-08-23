//! Compatibility tests for schema-1 evidence receipts.

use super::{RunStatus, TaskStatus, TerminationReason};

#[test]
fn stable_status_and_termination_values_are_lowercase() {
    for (value, expected) in [
        (serde_json::to_string(&RunStatus::Passed), r#""passed""#),
        (serde_json::to_string(&RunStatus::Failed), r#""failed""#),
        (
            serde_json::to_string(&RunStatus::Cancelled),
            r#""cancelled""#,
        ),
        (serde_json::to_string(&TaskStatus::Blocked), r#""blocked""#),
        (
            serde_json::to_string(&TerminationReason::Timeout),
            r#""timeout""#,
        ),
        (
            serde_json::to_string(&TerminationReason::Interrupted),
            r#""interrupted""#,
        ),
    ] {
        assert_eq!(value.ok().as_deref(), Some(expected));
    }
}
