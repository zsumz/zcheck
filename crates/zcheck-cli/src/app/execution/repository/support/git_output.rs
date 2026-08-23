//! Checked Git command result classification.

use std::process::Output;

use crate::app::error::AppError;

pub(super) fn checked(arguments: &[&str], output: Output) -> Result<Output, AppError> {
    if output.status.success() {
        return Ok(output);
    }
    Err(AppError::configuration(format!(
        "git {} failed: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}
