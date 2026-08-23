//! Validated task names used as graph identities.

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Error;

/// A task name matching `[a-z][a-z0-9_-]*`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskName(String);

impl TaskName {
    /// Returns the validated task name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for TaskName {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let mut bytes = source.bytes();
        let valid_start = bytes.next().is_some_and(|byte| byte.is_ascii_lowercase());
        let valid_tail = bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        });
        if !valid_start || !valid_tail {
            return Err(Error::new(format!(
                "invalid task name `{source}`: expected [a-z][a-z0-9_-]*"
            )));
        }
        Ok(Self(source.to_owned()))
    }
}

impl Display for TaskName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for TaskName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for TaskName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let source = String::deserialize(deserializer)?;
        source.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "name_test.rs"]
mod name_test;
