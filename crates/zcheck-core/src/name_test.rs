//! Tests for task-name identity rules.

use std::str::FromStr;

use super::TaskName;

#[test]
fn accepts_the_schema_shape() {
    for source in ["check", "rust-lint", "test_2", "a"] {
        assert!(TaskName::from_str(source).is_ok(), "rejected {source}");
    }
}

#[test]
fn rejects_names_that_could_weaken_lookup() {
    for source in ["", "Check", "2test", "rust.lint", "rust lint"] {
        assert!(TaskName::from_str(source).is_err(), "accepted {source}");
    }
}
