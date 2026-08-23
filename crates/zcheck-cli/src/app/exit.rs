//! Stable process exit codes for the published zcheck command.

pub(crate) const PASSED: i32 = 0;
pub(crate) const QUALIFICATION_FAILED: i32 = 1;
pub(crate) const CONFIGURATION: i32 = 2;
pub(crate) const INTERNAL: i32 = 3;
pub(crate) const INTERRUPTED: i32 = 130;

#[cfg(test)]
#[path = "exit_test.rs"]
mod exit_test;
