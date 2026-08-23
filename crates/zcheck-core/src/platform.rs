//! Explicit host-platform models for task projection.

use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

/// A supported task platform model.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// Linux hosts.
    Linux,
    /// Apple operating-system hosts.
    Macos,
    /// Windows hosts.
    Windows,
}

impl Platform {
    /// Returns the platform model for the compiling host when it is supported.
    #[must_use]
    pub const fn current() -> Option<Self> {
        from_target_flags(
            cfg!(target_os = "windows"),
            cfg!(target_os = "macos"),
            cfg!(target_os = "linux"),
        )
    }

    /// Returns the schema spelling for the platform.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }
}

const fn from_target_flags(windows: bool, macos: bool, linux: bool) -> Option<Platform> {
    if windows {
        Some(Platform::Windows)
    } else if macos {
        Some(Platform::Macos)
    } else if linux {
        Some(Platform::Linux)
    } else {
        None
    }
}

impl Display for Platform {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "platform_test.rs"]
mod platform_test;
