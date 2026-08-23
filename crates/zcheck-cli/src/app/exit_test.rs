//! Compatibility tests for the published process exit-code table.

use super::{CONFIGURATION, INTERNAL, INTERRUPTED, PASSED, QUALIFICATION_FAILED};

#[test]
fn stable_exit_codes_have_the_documented_values() {
    assert_eq!(PASSED, 0);
    assert_eq!(QUALIFICATION_FAILED, 1);
    assert_eq!(CONFIGURATION, 2);
    assert_eq!(INTERNAL, 3);
    assert_eq!(INTERRUPTED, 130);
}
