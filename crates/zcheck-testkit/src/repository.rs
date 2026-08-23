//! Isolated repository and cache directories for runner tests.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

/// A temporary repository with a separate zcheck cache directory.
#[derive(Debug)]
pub struct RepositoryFixture {
    parent: PathBuf,
    repository: PathBuf,
    cache: PathBuf,
}

impl RepositoryFixture {
    /// Creates an isolated repository containing the supplied manifest.
    pub fn new(manifest: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let parent =
            std::env::temp_dir().join(format!("zcheck-testkit-{}-{sequence}", std::process::id()));
        let repository = parent.join("repository");
        let cache = parent.join("cache");
        assert!(
            fs::create_dir_all(&repository).is_ok(),
            "testkit repository must be creatable"
        );
        assert!(
            fs::create_dir_all(&cache).is_ok(),
            "testkit cache must be creatable"
        );
        assert!(
            fs::write(repository.join("zcheck.toml"), manifest).is_ok(),
            "testkit manifest must be writable"
        );
        Self {
            parent,
            repository,
            cache,
        }
    }

    /// Creates a command for a zcheck binary rooted in this repository.
    pub fn command(&self, binary: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(binary);
        command
            .current_dir(&self.repository)
            .env("ZCHECK_CACHE_DIR", &self.cache);
        command
    }

    /// Returns the temporary repository root.
    #[must_use]
    pub fn repository(&self) -> &Path {
        &self.repository
    }

    /// Returns the temporary cache root.
    #[must_use]
    pub fn cache(&self) -> &Path {
        &self.cache
    }

    /// Creates one repository-relative directory for a fixture task.
    pub fn create_repository_directory(&self, path: impl AsRef<Path>) -> bool {
        fs::create_dir_all(self.repository.join(path)).is_ok()
    }

    /// Writes one repository-relative fixture file.
    pub fn write_repository_file(&self, path: impl AsRef<Path>, contents: &str) -> bool {
        let target = self.repository.join(path);
        target
            .parent()
            .is_some_and(|parent| fs::create_dir_all(parent).is_ok())
            && fs::write(target, contents).is_ok()
    }

    /// Initializes and commits the current fixture contents as a Git worktree.
    pub fn initialize_git(&self) -> bool {
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

    /// Reads a UTF-8 fixture file when it belongs to the repository or cache.
    pub fn read_to_string(&self, path: impl AsRef<Path>) -> Option<String> {
        let path = path.as_ref();
        if !path.starts_with(&self.repository) && !path.starts_with(&self.cache) {
            return None;
        }
        fs::read_to_string(path).ok()
    }

    /// Returns Unix permission bits for a fixture-owned path.
    #[cfg(unix)]
    pub fn unix_mode(&self, path: impl AsRef<Path>) -> Option<u32> {
        use std::os::unix::fs::PermissionsExt;

        let path = path.as_ref();
        if !path.starts_with(&self.repository) && !path.starts_with(&self.cache) {
            return None;
        }
        fs::metadata(path)
            .ok()
            .map(|metadata| metadata.permissions().mode() & 0o777)
    }

    /// Copies one executable fixture into the cache outside the repository.
    pub fn copy_to_cache(&self, source: impl AsRef<Path>, name: &str) -> Option<PathBuf> {
        let target = self.cache.join(name);
        fs::copy(source, &target).ok().map(|_| target)
    }

    /// Writes one fixture file outside the repository in its isolated cache.
    pub fn write_cache_file(&self, name: &str, contents: &str) -> Option<PathBuf> {
        let target = self.cache.join(name);
        fs::write(&target, contents).ok().map(|()| target)
    }

    /// Creates a repository-local symlink for Unix containment tests.
    #[cfg(unix)]
    pub fn symlink_into_repository(
        &self,
        target: impl AsRef<Path>,
        link: impl AsRef<Path>,
    ) -> bool {
        std::os::unix::fs::symlink(target, self.repository.join(link)).is_ok()
    }

    /// Replaces a repository file with a symlink for trust-boundary tests.
    #[cfg(unix)]
    pub fn replace_repository_file_with_symlink(
        &self,
        target: impl AsRef<Path>,
        link: impl AsRef<Path>,
    ) -> bool {
        let link = self.repository.join(link);
        fs::remove_file(&link).is_ok() && std::os::unix::fs::symlink(target, link).is_ok()
    }
}

impl Drop for RepositoryFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.parent).ok();
    }
}
