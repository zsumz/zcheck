//! Linux process-table parsing tests for dead-only task groups.

#![cfg(target_os = "linux")]

use super::process_state_and_group;

#[test]
fn linux_process_states_distinguish_zombies_from_live_members() {
    assert_eq!(
        process_state_and_group("123 (cargo test) S 42 123 123 0"),
        Some((b'S', 123))
    );
    assert_eq!(
        process_state_and_group("124 (name with ) inside) Z 1 123 123 0"),
        Some((b'Z', 123))
    );
    assert_eq!(process_state_and_group("not proc stat"), None);
}
