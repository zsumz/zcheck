//! Operating-system cache discovery and unique run-directory allocation.

use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use super::super::super::error::AppError;

struct CacheLocation {
    root: PathBuf,
    prefix: &'static str,
    create_root: bool,
}

pub(super) fn default_run_directory(run_id: &str) -> Result<PathBuf, AppError> {
    let location = cache_location();
    if location.create_root {
        fs::create_dir_all(&location.root).map_err(|error| {
            AppError::internal(format!(
                "cannot create log directory root {}: {error}",
                location.root.display()
            ))
        })?;
    }
    for attempt in 0_u8..100 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let directory = location
            .root
            .join(format!("{}-{}{suffix}", location.prefix, &run_id[4..]));
        match super::create_private_directory(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(AppError::internal(format!(
                    "cannot create log directory {}: {error}",
                    directory.display()
                )));
            }
        }
    }
    Err(AppError::internal(
        "cannot allocate a unique task-log directory",
    ))
}

fn cache_location() -> CacheLocation {
    if let Some(path) = env::var_os("ZCHECK_CACHE_DIR") {
        return CacheLocation {
            root: PathBuf::from(path),
            prefix: "run",
            create_root: true,
        };
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = env::var_os("HOME") {
        return CacheLocation {
            root: PathBuf::from(home).join("Library/Caches/zcheck"),
            prefix: "run",
            create_root: true,
        };
    }
    #[cfg(windows)]
    if let Some(path) = env::var_os("LOCALAPPDATA") {
        return CacheLocation {
            root: PathBuf::from(path).join("zcheck"),
            prefix: "run",
            create_root: true,
        };
    }
    #[cfg(not(windows))]
    if let Some(path) = env::var_os("XDG_CACHE_HOME") {
        return CacheLocation {
            root: PathBuf::from(path).join("zcheck"),
            prefix: "run",
            create_root: true,
        };
    }
    #[cfg(not(windows))]
    if let Some(home) = env::var_os("HOME") {
        return CacheLocation {
            root: PathBuf::from(home).join(".cache/zcheck"),
            prefix: "run",
            create_root: true,
        };
    }
    CacheLocation {
        root: env::temp_dir(),
        prefix: "zcheck-run",
        create_root: false,
    }
}
