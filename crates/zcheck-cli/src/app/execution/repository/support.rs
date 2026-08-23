//! Git command and untracked-file support for repository fingerprints.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

use super::{capture_repository, display_path, hex};
use crate::app::error::AppError;

mod git_output;
mod path;
mod tracked;

type UntrackedEntries = BTreeMap<Vec<u8>, String>;

pub(super) fn tracked(
    root: &Path,
    excluded: &[PathBuf],
) -> Result<BTreeMap<Vec<u8>, String>, AppError> {
    tracked::capture(root, excluded)
}

fn tracked_worktree(root: &Path, raw: &[u8], hash: &mut Sha256) -> Result<(), AppError> {
    let path = root.join(path_from_git(raw));
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            hash.update(b"symlink\0");
            let target = fs::read_link(&path).map_err(|error| {
                AppError::internal(format!("cannot read symlink {}: {error}", path.display()))
            })?;
            hash.update(os_bytes(&target));
        }
        Ok(metadata) if metadata.is_file() => {
            hash.update(b"file\0");
            hash.update(mode_bytes(&metadata));
            hash_file(&path, hash)?;
        }
        Ok(metadata) if metadata.is_dir() => {
            hash.update(b"directory\0");
            let resolved = fs::canonicalize(&path).map_err(|error| {
                AppError::configuration(format!(
                    "cannot resolve tracked directory {}: {error}",
                    path.display()
                ))
            })?;
            let nested = capture_repository(&resolved, &[])?;
            hash.update(nested.evidence.sha256.as_bytes());
        }
        Ok(_) => {
            return Err(AppError::configuration(format!(
                "tracked path {} has an unsupported file type",
                path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => hash.update(b"absent\0"),
        Err(error) => {
            return Err(AppError::internal(format!(
                "cannot inspect tracked path {}: {error}",
                path.display()
            )));
        }
    }
    Ok(())
}

pub(super) fn untracked(
    root: &Path,
    excluded: &[PathBuf],
) -> Result<(String, UntrackedEntries), AppError> {
    let listed = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let mut total = Sha256::new();
    let mut entries = BTreeMap::new();
    for raw in listed
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = path_from_git(raw);
        if excluded.iter().any(|excluded| path.starts_with(excluded)) {
            continue;
        }
        let entry = hash_entry(&root.join(&path), raw)?;
        total.update(raw.len().to_le_bytes());
        total.update(raw);
        total.update(entry.as_bytes());
        entries.insert(raw.to_vec(), entry);
    }
    Ok((hex(total.finalize()), entries))
}

fn hash_entry(path: &Path, raw: &[u8]) -> Result<String, AppError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        AppError::internal(format!(
            "cannot inspect untracked path {}: {error}",
            display_path(raw)
        ))
    })?;
    let mut hash = Sha256::new();
    if metadata.file_type().is_symlink() {
        hash.update(b"symlink\0");
        hash.update(os_bytes(&fs::read_link(path).map_err(|error| {
            AppError::internal(format!("cannot read symlink {}: {error}", path.display()))
        })?));
    } else if metadata.is_file() {
        hash.update(b"file\0");
        hash.update(mode_bytes(&metadata));
        hash_file(path, &mut hash)?;
    } else {
        return Err(AppError::configuration(format!(
            "untracked path {} is neither a file nor a symlink",
            path.display()
        )));
    }
    Ok(hex(hash.finalize()))
}

fn hash_file(path: &Path, hash: &mut Sha256) -> Result<(), AppError> {
    let mut file = File::open(path).map_err(|error| {
        AppError::internal(format!(
            "cannot read untracked file {}: {error}",
            path.display()
        ))
    })?;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            AppError::internal(format!(
                "cannot hash untracked file {}: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            return Ok(());
        }
        hash.update(&buffer[..read]);
    }
}

pub(super) fn submodules(root: &Path) -> Result<(String, bool), AppError> {
    let index = git(root, &["ls-files", "--stage", "-z"])?;
    let mut hash = Sha256::new();
    let mut clean = true;
    for record in index
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty())
    {
        let Some(separator) = record.iter().position(|byte| *byte == b'\t') else {
            continue;
        };
        let (header, tail) = record.split_at(separator);
        let raw_path = &tail[1..];
        if !header.starts_with(b"160000 ") {
            continue;
        }
        hash.update(raw_path.len().to_le_bytes());
        hash.update(raw_path);
        let path = root.join(path_from_git(raw_path));
        if path.is_dir() {
            let resolved = fs::canonicalize(&path).map_err(|error| {
                AppError::configuration(format!(
                    "cannot resolve submodule {}: {error}",
                    path.display()
                ))
            })?;
            let nested = capture_repository(&resolved, &[])?;
            hash.update(nested.evidence.sha256.as_bytes());
            clean &= nested.evidence.clean;
        } else {
            hash.update(b"uninitialized");
        }
    }
    Ok((hex(hash.finalize()), clean))
}

pub(super) fn git(root: &Path, arguments: &[&str]) -> Result<Vec<u8>, AppError> {
    let output = command(root, arguments)?;
    checked_git(arguments, output).map(|value| value.stdout)
}

pub(super) fn optional_git(root: &Path, arguments: &[&str]) -> Result<Option<Vec<u8>>, AppError> {
    let output = command(root, arguments)?;
    if output.status.success() {
        Ok(Some(output.stdout))
    } else if output.status.code() == Some(1) {
        Ok(None)
    } else {
        checked_git(arguments, output).map(|_| None)
    }
}

fn command(root: &Path, arguments: &[&str]) -> Result<Output, AppError> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|error| AppError::configuration(format!("cannot execute git: {error}")))
}

fn checked_git(arguments: &[&str], output: Output) -> Result<Output, AppError> {
    git_output::checked(arguments, output)
}

fn path_from_git(bytes: &[u8]) -> PathBuf {
    path::from_git(bytes)
}

#[cfg(unix)]
fn os_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn os_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(unix)]
fn mode_bytes(metadata: &fs::Metadata) -> [u8; 4] {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode().to_le_bytes()
}

#[cfg(windows)]
fn mode_bytes(metadata: &fs::Metadata) -> [u8; 1] {
    [u8::from(metadata.permissions().readonly())]
}
