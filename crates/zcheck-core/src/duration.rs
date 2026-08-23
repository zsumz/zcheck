//! Bounded duration syntax used by manifest timeouts.

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Error;

/// A positive integer duration with an explicit `ms`, `s`, `m`, or `h` suffix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurationSpec {
    source: String,
    milliseconds: u64,
}

impl DurationSpec {
    /// Returns the duration in milliseconds.
    #[must_use]
    pub const fn milliseconds(&self) -> u64 {
        self.milliseconds
    }

    /// Returns the canonical source representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }
}

impl FromStr for DurationSpec {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let (number, multiplier) = suffix(source).ok_or_else(|| {
            Error::new(format!(
                "invalid duration `{source}`: expected a positive integer followed by ms, s, m, or h"
            ))
        })?;
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::new(format!(
                "invalid duration `{source}`: expected a positive integer followed by ms, s, m, or h"
            )));
        }
        let value = number
            .parse::<u64>()
            .map_err(|_| Error::new(format!("duration `{source}` is too large")))?;
        if value == 0 {
            return Err(Error::new(format!("duration `{source}` must be positive")));
        }
        let milliseconds = value
            .checked_mul(multiplier)
            .ok_or_else(|| Error::new(format!("duration `{source}` is too large")))?;
        Ok(Self {
            source: source.to_owned(),
            milliseconds,
        })
    }
}

impl Display for DurationSpec {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source)
    }
}

impl Serialize for DurationSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.source)
    }
}

impl<'de> Deserialize<'de> for DurationSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let source = String::deserialize(deserializer)?;
        source.parse().map_err(serde::de::Error::custom)
    }
}

fn suffix(source: &str) -> Option<(&str, u64)> {
    source
        .strip_suffix("ms")
        .map(|number| (number, 1))
        .or_else(|| source.strip_suffix('s').map(|number| (number, 1_000)))
        .or_else(|| source.strip_suffix('m').map(|number| (number, 60_000)))
        .or_else(|| source.strip_suffix('h').map(|number| (number, 3_600_000)))
}

#[cfg(test)]
#[path = "duration_test.rs"]
mod duration_test;
