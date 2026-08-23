//! Stable command-line error classification.

use std::fmt::{self, Display, Formatter};

use super::exit;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ErrorKind {
    Usage,
    Configuration,
    Internal,
}

#[derive(Debug)]
pub(crate) struct AppError {
    kind: ErrorKind,
    message: String,
}

impl AppError {
    pub(crate) fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Usage,
            message: message.into(),
        }
    }

    pub(crate) fn configuration(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Configuration,
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Internal,
            message: message.into(),
        }
    }

    pub(crate) const fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::Usage | ErrorKind::Configuration => exit::CONFIGURATION,
            ErrorKind::Internal => exit::INTERNAL,
        }
    }
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<zcheck_core::Error> for AppError {
    fn from(error: zcheck_core::Error) -> Self {
        Self::configuration(error.to_string())
    }
}
