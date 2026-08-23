//! Non-interleaving live progress for concurrent human runs.

use zcheck_core::{TaskName, TaskResult};

use super::model::status_label;

pub(super) struct Reporter {
    enabled: bool,
}

impl Reporter {
    pub(super) const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub(super) fn started(&self, task: &TaskName) {
        if self.enabled {
            eprintln!("START  {task}");
        }
    }

    pub(super) fn finished(&self, result: &TaskResult) {
        if self.enabled {
            eprintln!(
                "{:<6} {}  {}",
                status_label(result.status),
                result.name,
                duration(result.duration_ms)
            );
        }
    }
}

fn duration(milliseconds: u64) -> String {
    let tenths = milliseconds / 100;
    format!("{}.{}s", tenths / 10, tenths % 10)
}
