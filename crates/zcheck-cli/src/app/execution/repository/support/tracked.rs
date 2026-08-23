//! Per-path fingerprints for exact tracked-change diagnostics.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{git, path_from_git};
use crate::app::error::AppError;

pub(super) fn capture(
    root: &Path,
    excluded: &[PathBuf],
) -> Result<BTreeMap<Vec<u8>, String>, AppError> {
    let mut paths = BTreeSet::new();
    for arguments in [
        &[
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--ignore-submodules=none",
        ][..],
        &["diff", "--name-only", "-z", "--ignore-submodules=none"][..],
    ] {
        for raw in git(root, arguments)?
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = path_from_git(raw);
            if !excluded.iter().any(|excluded| path.starts_with(excluded)) {
                paths.insert(raw.to_vec());
            }
        }
    }
    let index = index_entries(root)?;
    paths
        .into_iter()
        .map(|raw| {
            path_fingerprint(root, &raw, index.get(&raw).map(Vec::as_slice)).map(|hash| (raw, hash))
        })
        .collect()
}

fn index_entries(root: &Path) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, AppError> {
    let mut entries = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for record in git(root, &["ls-files", "--stage", "-z"])?
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let Some(separator) = record.iter().position(|byte| *byte == b'\t') else {
            continue;
        };
        let raw = record[separator + 1..].to_vec();
        let entry = entries.entry(raw).or_default();
        entry.extend_from_slice(&record.len().to_le_bytes());
        entry.extend_from_slice(record);
    }
    Ok(entries)
}

fn path_fingerprint(root: &Path, raw: &[u8], index: Option<&[u8]>) -> Result<String, AppError> {
    let mut hash = Sha256::new();
    hash.update(b"index\0");
    if let Some(entry) = index {
        hash.update(entry);
    }
    super::tracked_worktree(root, raw, &mut hash)?;
    Ok(super::super::hex(hash.finalize()))
}
