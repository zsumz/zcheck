//! Focused tests for content-correct Git fingerprints.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::capture;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    parent: PathBuf,
    repository: PathBuf,
    #[cfg(unix)]
    cache: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "zcheck-repository-test-{}-{sequence}",
            std::process::id()
        ));
        let repository = parent.join("repository");
        assert!(fs::create_dir_all(&repository).is_ok());
        #[cfg(unix)]
        let cache = parent.join("cache");
        #[cfg(unix)]
        assert!(fs::create_dir_all(&cache).is_ok());
        assert!(
            fs::write(
                repository.join("zcheck.toml"),
                "schema = 1\n[tasks.check]\nrun = [\"git\"]\n"
            )
            .is_ok()
        );
        Self {
            parent,
            repository,
            #[cfg(unix)]
            cache,
        }
    }

    fn repository(&self) -> &Path {
        &self.repository
    }

    fn write(&self, path: &str, contents: &str) -> bool {
        fs::write(self.repository.join(path), contents).is_ok()
    }

    fn initialize_git(&self) -> bool {
        for arguments in [
            &["init", "--quiet"][..],
            &["config", "user.name", "zsumz"][..],
            &["config", "user.email", "shawn@zsumz.com"][..],
            &["config", "core.autocrlf", "false"][..],
            &["add", "."][..],
            &["commit", "--quiet", "--no-gpg-sign", "-m", "fixture"][..],
        ] {
            if !Command::new("git")
                .current_dir(&self.repository)
                .args(arguments)
                .status()
                .is_ok_and(|status| status.success())
            {
                return false;
            }
        }
        true
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.parent).ok();
    }
}

#[test]
fn untracked_file_contents_change_the_fingerprint() {
    let fixture = Fixture::new();
    assert!(fixture.initialize_git());
    assert!(fixture.write("untracked.txt", "before\n"));
    let before = capture(fixture.repository(), &[]);
    assert!(fixture.write("untracked.txt", "after\n"));
    let after = capture(fixture.repository(), &[]);
    assert!(matches!((before, after), (Ok(left), Ok(right)) if !left.preserved(&right)));
}

#[test]
fn diagnostics_omit_unchanged_preexisting_dirty_paths() {
    let fixture = Fixture::new();
    assert!(fixture.write("unchanged.txt", "clean\n"));
    assert!(fixture.write("changed.txt", "clean\n"));
    assert!(fixture.initialize_git());
    assert!(fixture.write("unchanged.txt", "dirty before\n"));
    assert!(fixture.write("changed.txt", "dirty before\n"));
    let before = capture(fixture.repository(), &[]);
    assert!(fixture.write("changed.txt", "dirty after\n"));
    let after = capture(fixture.repository(), &[]);
    let changes = before
        .as_ref()
        .ok()
        .zip(after.as_ref().ok())
        .map(|(left, right)| left.changes(right));
    assert_eq!(changes, Some(vec!["modified: changed.txt".to_owned()]));
}

#[test]
fn diagnostics_detect_index_changes_when_worktree_content_is_stable() {
    let fixture = Fixture::new();
    assert!(fixture.write("tracked.txt", "clean\n"));
    assert!(fixture.initialize_git());
    assert!(fixture.write("tracked.txt", "staged before\n"));
    let staged = Command::new("git")
        .current_dir(fixture.repository())
        .args(["add", "tracked.txt"])
        .status();
    assert!(staged.is_ok_and(|status| status.success()));
    assert!(fixture.write("tracked.txt", "stable worktree\n"));
    let before = capture(fixture.repository(), &[]);
    let restaged = Command::new("git")
        .current_dir(fixture.repository())
        .args(["add", "tracked.txt"])
        .status();
    assert!(restaged.is_ok_and(|status| status.success()));
    let after = capture(fixture.repository(), &[]);
    let changes = before
        .as_ref()
        .ok()
        .zip(after.as_ref().ok())
        .map(|(left, right)| left.changes(right));
    assert_eq!(changes, Some(vec!["modified: tracked.txt".to_owned()]));
}

#[cfg(unix)]
#[test]
fn untracked_symlink_targets_change_the_fingerprint() {
    let fixture = Fixture::new();
    assert!(fixture.initialize_git());
    let first = fixture.cache.join("first");
    let second = fixture.cache.join("second");
    assert!(fs::write(&first, "first\n").is_ok());
    assert!(fs::write(&second, "second\n").is_ok());
    assert!(std::os::unix::fs::symlink(&first, fixture.repository.join("untracked-link")).is_ok());
    let before = capture(fixture.repository(), &[]);
    assert!(fs::remove_file(fixture.repository().join("untracked-link")).is_ok());
    assert!(std::os::unix::fs::symlink(&second, fixture.repository.join("untracked-link")).is_ok());
    let after = capture(fixture.repository(), &[]);
    assert!(matches!((before, after), (Ok(left), Ok(right)) if !left.preserved(&right)));
}

#[test]
fn dirty_submodule_content_changes_the_recursive_fingerprint() {
    let child = Fixture::new();
    assert!(child.write("tracked.txt", "clean\n"));
    assert!(child.initialize_git());
    let parent = Fixture::new();
    assert!(parent.initialize_git());
    let added = Command::new("git")
        .current_dir(parent.repository())
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--quiet",
        ])
        .arg(child.repository())
        .arg("child")
        .status();
    assert!(added.is_ok_and(|status| status.success()));
    let committed = Command::new("git")
        .current_dir(parent.repository())
        .args(["commit", "--quiet", "--no-gpg-sign", "-am", "add child"])
        .status();
    assert!(committed.is_ok_and(|status| status.success()));
    assert!(fs::write(parent.repository().join("child/tracked.txt"), "dirty one\n").is_ok());
    let before = capture(parent.repository(), &[]);
    assert!(fs::write(parent.repository().join("child/tracked.txt"), "dirty two\n").is_ok());
    let after = capture(parent.repository(), &[]);
    assert!(matches!((before, after), (Ok(left), Ok(right)) if !left.preserved(&right)));
}
