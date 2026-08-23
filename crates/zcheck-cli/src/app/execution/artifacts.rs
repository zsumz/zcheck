//! Explicit and default artifact locations for one run.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use super::super::error::AppError;

#[derive(Debug, Default)]
pub(crate) struct ArtifactOptions {
    pub(crate) logs_dir: Option<PathBuf>,
    pub(crate) receipt: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) struct ArtifactPaths {
    pub(super) logs_dir: Option<PathBuf>,
    pub(super) receipt: Option<PathBuf>,
}

impl ArtifactOptions {
    pub(super) fn resolve(self, root: &Path) -> Result<ArtifactPaths, AppError> {
        Ok(ArtifactPaths {
            logs_dir: self
                .logs_dir
                .as_deref()
                .map(|path| resolve(root, path))
                .transpose()?,
            receipt: self
                .receipt
                .as_deref()
                .map(|path| resolve(root, path))
                .transpose()?,
        })
    }
}

fn resolve(root: &Path, path: &Path) -> Result<PathBuf, AppError> {
    if path.as_os_str().is_empty() {
        return Err(AppError::usage("artifact paths must not be empty"));
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(AppError::usage(format!(
            "relative artifact path `{}` may not escape the repository",
            path.display()
        )));
    }
    let root = fs::canonicalize(root).map_err(|error| {
        AppError::configuration(format!("cannot resolve repository root: {error}"))
    })?;
    let candidate = root.join(path);
    let (existing, suffix) = existing_ancestor(&candidate)?;
    let resolved = fs::canonicalize(existing).map_err(|error| {
        AppError::configuration(format!(
            "cannot resolve artifact path `{}`: {error}",
            path.display()
        ))
    })?;
    if !resolved.starts_with(&root) {
        return Err(AppError::usage(format!(
            "relative artifact path `{}` resolves outside the repository",
            path.display()
        )));
    }
    Ok(resolved.join(suffix))
}

fn existing_ancestor(path: &Path) -> Result<(&Path, PathBuf), AppError> {
    let mut existing = path;
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => {
                let suffix =
                    path.strip_prefix(existing)
                        .map(Path::to_path_buf)
                        .map_err(|error| {
                            AppError::internal(format!("cannot resolve artifact suffix: {error}"))
                        })?;
                return Ok((existing, suffix));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                existing = existing.parent().ok_or_else(|| {
                    AppError::configuration("artifact path has no existing ancestor")
                })?;
            }
            Err(error) => {
                return Err(AppError::configuration(format!(
                    "cannot inspect artifact path {}: {error}",
                    existing.display()
                )));
            }
        }
    }
}
