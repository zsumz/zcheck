//! Exact Git checkout-state fingerprints for repository qualification.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zcheck_core::{RepositorySnapshot, RepositoryStatePolicy};

use super::super::error::AppError;
use support::{git, optional_git, submodules, tracked, untracked};

mod support;

#[derive(Debug)]
pub(super) struct CapturedState {
    evidence: RepositorySnapshot,
    tracked: BTreeMap<Vec<u8>, String>,
    untracked: BTreeMap<Vec<u8>, String>,
}

impl CapturedState {
    pub(super) const fn evidence(&self) -> &RepositorySnapshot {
        &self.evidence
    }

    pub(super) fn preserved(&self, after: &Self) -> bool {
        self.evidence.sha256 == after.evidence.sha256
    }

    pub(super) fn changes(&self, after: &Self) -> Vec<String> {
        let mut changes = Vec::new();
        if self.evidence.head != after.evidence.head {
            changes.push("modified: HEAD".to_owned());
        }
        let paths = self
            .tracked
            .keys()
            .chain(after.tracked.keys())
            .chain(self.untracked.keys())
            .chain(after.untracked.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for path in paths {
            let before = self.path_state(&path);
            let current = after.path_state(&path);
            if before == current {
                continue;
            }
            let kind = match (before, current) {
                (None, Some(PathState::Untracked(_))) => "added",
                (Some(PathState::Untracked(_)), None) => "removed",
                _ => "modified",
            };
            changes.push(format!("{kind}: {}", display_path(&path)));
        }
        changes.sort();
        changes.dedup();
        changes
    }

    fn path_state(&self, path: &[u8]) -> Option<PathState<'_>> {
        self.tracked
            .get(path)
            .map(|hash| PathState::Tracked(hash))
            .or_else(|| {
                self.untracked
                    .get(path)
                    .map(|hash| PathState::Untracked(hash))
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PathState<'a> {
    Tracked(&'a str),
    Untracked(&'a str),
}

pub(super) fn capture(root: &Path, excluded: &[PathBuf]) -> Result<CapturedState, AppError> {
    let canonical_root = repository_root(root)?;
    capture_repository(&canonical_root, excluded)
}

fn capture_repository(root: &Path, excluded: &[PathBuf]) -> Result<CapturedState, AppError> {
    let head = optional_git(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?
        .map(|bytes| trim_line(&bytes));
    let staged = git(
        root,
        &[
            "diff",
            "--cached",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ],
    )?;
    let unstaged = git(
        root,
        &[
            "diff",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ],
    )?;
    let (untracked_hash, untracked) = untracked(root, excluded)?;
    let (submodules_hash, submodules_clean) = submodules(root)?;
    let staged_hash = digest(&staged);
    let unstaged_hash = digest(&unstaged);
    let mut combined = Sha256::new();
    component(&mut combined, "head", head.as_deref().unwrap_or(""));
    component(&mut combined, "staged", &staged_hash);
    component(&mut combined, "unstaged", &unstaged_hash);
    component(&mut combined, "untracked", &untracked_hash);
    component(&mut combined, "submodules", &submodules_hash);
    let clean =
        staged.is_empty() && unstaged.is_empty() && untracked.is_empty() && submodules_clean;
    let tracked = tracked(root, excluded)?;
    Ok(CapturedState {
        evidence: RepositorySnapshot {
            sha256: hex(combined.finalize()),
            head,
            staged_sha256: staged_hash,
            unstaged_sha256: unstaged_hash,
            untracked_sha256: untracked_hash,
            submodules_sha256: submodules_hash,
            clean,
        },
        tracked,
        untracked,
    })
}

fn repository_root(root: &Path) -> Result<PathBuf, AppError> {
    let top = trim_line(&git(root, &["rev-parse", "--show-toplevel"])?);
    let resolved = fs::canonicalize(&top).map_err(|error| {
        AppError::configuration(format!("cannot resolve Git root `{top}`: {error}"))
    })?;
    let expected = fs::canonicalize(root).map_err(|error| {
        AppError::configuration(format!("cannot resolve repository root: {error}"))
    })?;
    if resolved != expected {
        return Err(AppError::configuration(format!(
            "zcheck root {} is not the Git worktree root {}",
            expected.display(),
            resolved.display()
        )));
    }
    Ok(resolved)
}

fn component(hash: &mut Sha256, name: &str, value: &str) {
    hash.update(name.as_bytes());
    hash.update([0]);
    hash.update(value.len().to_le_bytes());
    hash.update(value.as_bytes());
}

pub(super) fn digest(bytes: &[u8]) -> String {
    hex(Sha256::digest(bytes))
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn trim_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}

pub(super) fn display_path(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub(crate) fn inspect_repository_state(
    root: &Path,
    policy: RepositoryStatePolicy,
) -> Result<(bool, String), AppError> {
    if policy == RepositoryStatePolicy::Ignore {
        return Ok((true, "Git inspection is disabled".to_owned()));
    }
    let state = capture(root, &[])?;
    let accepted = policy != RepositoryStatePolicy::Clean || state.evidence().clean;
    let detail = match policy {
        RepositoryStatePolicy::Preserve => "exact before/after preservation is enforced".to_owned(),
        RepositoryStatePolicy::Clean if accepted => {
            "the checkout is clean and clean-state enforcement is active".to_owned()
        }
        RepositoryStatePolicy::Clean => {
            "the checkout is dirty but clean state is required".to_owned()
        }
        RepositoryStatePolicy::Ignore => "Git inspection is disabled".to_owned(),
    };
    Ok((accepted, detail))
}

#[cfg(test)]
#[path = "repository_test.rs"]
mod repository_test;
