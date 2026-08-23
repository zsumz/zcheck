//! Git path-byte conversion without filesystem access.

use std::path::PathBuf;

#[cfg(unix)]
pub(super) fn from_git(bytes: &[u8]) -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    PathBuf::from(OsString::from_vec(bytes.to_vec()))
}

#[cfg(windows)]
pub(super) fn from_git(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}
