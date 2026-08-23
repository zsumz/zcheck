//! Tests for fail-closed host-platform selection.

use super::{Platform, from_target_flags};

#[test]
fn maps_only_the_three_supported_host_families() {
    assert_eq!(
        from_target_flags(true, false, false),
        Some(Platform::Windows)
    );
    assert_eq!(from_target_flags(false, true, false), Some(Platform::Macos));
    assert_eq!(from_target_flags(false, false, true), Some(Platform::Linux));
    assert_eq!(from_target_flags(false, false, false), None);
}

#[test]
fn recognizes_the_current_test_host() {
    assert!(Platform::current().is_some());
}
