//! Child behavior containing workflow-command-shaped task output.

use std::env;

use super::ROLE;

pub(super) const TEST_NAME: &str = "fixture::github_reporter_fixture";

#[test]
fn github_reporter_fixture() {
    let Some(role) = env::var(ROLE).ok() else {
        return;
    };
    println!("::error title=injected::payload%\nsecond line");
    assert_eq!(role, "pass", "intentional GitHub reporter fixture failure");
}
