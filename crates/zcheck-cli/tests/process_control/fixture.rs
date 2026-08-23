//! Executable behaviors used by the process-control integration tests.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use super::{HEARTBEAT, LATER_MARKER, ROLE};

pub(super) const TEST_NAME: &str = "fixture::process_fixture";

#[test]
fn process_fixture() {
    match env::var(ROLE).ok().as_deref() {
        Some("large-output") => large_output(),
        Some("graceful") => graceful(),
        Some("orphan-parent") => spawn_child(),
        Some("tree-parent") => tree_parent(),
        Some("tree-child") => heartbeat(),
        Some("later") => later_marker(),
        _ => {}
    }
}

fn large_output() {
    let bytes = vec![b'x'; 64 * 1024];
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    for _ in 0..32 {
        assert!(stdout.write_all(&bytes).is_ok());
        assert!(stderr.write_all(&bytes).is_ok());
    }
    assert!(stdout.flush().is_ok());
    assert!(stderr.flush().is_ok());
}

#[cfg(unix)]
fn graceful() {
    use signal_hook::consts::SIGTERM;

    let stopped = Arc::new(AtomicBool::new(false));
    let registration = signal_hook::flag::register(SIGTERM, Arc::clone(&stopped));
    assert!(registration.is_ok());
    while !stopped.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn graceful() {}

fn ignore_termination() {
    #[cfg(unix)]
    {
        use signal_hook::consts::SIGTERM;

        let ignored = Arc::new(AtomicBool::new(false));
        let registration = signal_hook::flag::register(SIGTERM, ignored);
        assert!(registration.is_ok());
    }
}

fn tree_parent() {
    ignore_termination();
    spawn_child();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn spawn_child() {
    let Ok(helper) = env::current_exe() else {
        return;
    };
    let Some(path) = env::var_os(HEARTBEAT) else {
        return;
    };
    let child = Command::new(helper)
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(ROLE, "tree-child")
        .env(HEARTBEAT, path)
        .spawn();
    assert!(child.is_ok());
}

fn heartbeat() {
    ignore_termination();
    let Some(path) = env::var_os(HEARTBEAT).map(PathBuf::from) else {
        return;
    };
    for sequence in 0_u64.. {
        assert!(fs::write(&path, sequence.to_string()).is_ok());
        thread::sleep(Duration::from_millis(20));
    }
}

fn later_marker() {
    if let Some(path) = env::var_os(LATER_MARKER) {
        assert!(fs::write(path, "ran").is_ok());
    }
}
