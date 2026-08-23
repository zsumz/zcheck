//! Child-process behaviors for scheduler integration tests.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use super::{DELAY, EVENTS, MARKER, ROLE, TASK};

pub(super) const TEST_NAME: &str = "fixture::concurrency_fixture";

#[test]
fn concurrency_fixture() {
    let Some(role) = env::var(ROLE).ok() else {
        return;
    };
    let task = env::var(TASK).unwrap_or_default();
    event("start", &task);
    if let Some(marker) = env::var_os(MARKER) {
        assert!(fs::write(marker, &task).is_ok());
    }
    let delay = env::var(DELAY)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();
    thread::sleep(Duration::from_millis(delay));
    event("end", &task);
    assert_eq!(role, "work", "intentional scheduler fixture failure");
}

fn event(kind: &str, task: &str) {
    let Some(path) = env::var_os(EVENTS).map(PathBuf::from) else {
        return;
    };
    let file = OpenOptions::new().create(true).append(true).open(path);
    assert!(file.is_ok());
    let Some(mut file) = file.ok() else {
        return;
    };
    assert!(
        file.write_all(format!("{kind}:{task}\n").as_bytes())
            .is_ok()
    );
    assert!(file.flush().is_ok());
}
