//! Private persistence for stable evidence receipts.

use std::fs;
use std::io::Write;
use std::path::Path;

use zcheck_core::Receipt;

use super::super::error::AppError;
use super::logs::create_private_file;

pub(super) fn write(receipt: &Receipt, path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::internal(format!(
                "cannot create receipt directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let mut contents = receipt
        .to_json_pretty()
        .map_err(|error| AppError::internal(error.to_string()))?;
    contents.push('\n');
    let mut file = create_private_file(path).map_err(|error| {
        AppError::internal(format!("cannot create receipt {}: {error}", path.display()))
    })?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            AppError::internal(format!(
                "cannot persist receipt {}: {error}",
                path.display()
            ))
        })
}
