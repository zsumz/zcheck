//! Manifest location and upward repository discovery.

use std::fs;
use std::path::{Path, PathBuf};

use zcheck_core::Manifest;

use super::args::Location;
use super::error::AppError;

#[derive(Debug)]
pub(crate) struct Repository {
    pub(crate) root: PathBuf,
    pub(crate) manifest_path: PathBuf,
    pub(crate) manifest: Manifest,
}

pub(crate) fn load(location: &Location) -> Result<Repository, AppError> {
    let current = std::env::current_dir()
        .map_err(|error| AppError::internal(format!("cannot read current directory: {error}")))?;
    let manifest_path = if let Some(path) = &location.manifest {
        absolute(&current, path)
    } else if let Some(root) = &location.root {
        absolute(&current, root).join("zcheck.toml")
    } else {
        search_upward(&current)?
    };
    let declared_root = manifest_path
        .parent()
        .ok_or_else(|| AppError::configuration("manifest path has no repository parent"))?
        .to_path_buf();
    let root = fs::canonicalize(&declared_root).map_err(|error| {
        AppError::configuration(format!(
            "cannot resolve repository root {}: {error}",
            declared_root.display()
        ))
    })?;
    let manifest_path = fs::canonicalize(&manifest_path).map_err(|error| {
        AppError::configuration(format!("cannot resolve manifest path: {error}"))
    })?;
    if !manifest_path.starts_with(&root) {
        return Err(AppError::configuration(
            "zcheck.toml resolves outside its repository root",
        ));
    }
    let source = fs::read_to_string(&manifest_path).map_err(|error| {
        AppError::configuration(format!("cannot read {}: {error}", manifest_path.display()))
    })?;
    let manifest = Manifest::parse(&source)?;
    Ok(Repository {
        root,
        manifest_path,
        manifest,
    })
}

fn absolute(current: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current.join(path)
    }
}

fn search_upward(current: &Path) -> Result<PathBuf, AppError> {
    for directory in current.ancestors() {
        let candidate = directory.join("zcheck.toml");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(AppError::configuration(format!(
        "no zcheck.toml found from {} to the filesystem root",
        current.display()
    )))
}
