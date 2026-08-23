//! Strict TOML decoding for schema-1 manifests.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::{ExecutionPolicy, Manifest, Task};
use crate::{Error, TaskName};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema: u32,
    default: Option<TaskName>,
    #[serde(default)]
    execution: ExecutionPolicy,
    tasks: BTreeMap<TaskName, Task>,
}

impl Manifest {
    /// Parses and validates one complete schema-1 manifest.
    pub fn parse(source: &str) -> Result<Self, Error> {
        let raw = toml::from_str::<RawManifest>(source)
            .map_err(|error| Error::new(format!("invalid zcheck manifest: {error}")))?;
        let manifest = Self {
            schema: raw.schema,
            default: raw.default,
            execution: raw.execution,
            tasks: raw.tasks,
        };
        crate::validate::manifest(&manifest)?;
        Ok(manifest)
    }
}

#[cfg(test)]
#[path = "manifest_test.rs"]
mod manifest_test;
