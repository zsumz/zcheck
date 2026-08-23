//! Tests for stable run-status serialization.

use super::TaskStatus;

#[test]
fn serializes_the_complete_stable_status_vocabulary() {
    for (status, expected) in [
        (TaskStatus::Passed, r#""passed""#),
        (TaskStatus::Failed, r#""failed""#),
        (TaskStatus::Blocked, r#""blocked""#),
        (TaskStatus::Skipped, r#""skipped""#),
        (TaskStatus::Cancelled, r#""cancelled""#),
    ] {
        assert_eq!(
            serde_json::to_string(&status).ok().as_deref(),
            Some(expected)
        );
    }
}
