//! Concurrent pipe readers that stream complete output into one private log.

use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use super::super::super::error::AppError;
use super::super::logs::TaskLog;
use super::child::ManagedChild;

pub(super) struct Readers {
    stdout: JoinHandle<io::Result<()>>,
    stderr: JoinHandle<io::Result<()>>,
    failed: Arc<AtomicBool>,
}

impl Readers {
    pub(super) fn start(child: &mut ManagedChild, log: &TaskLog) -> Result<Self, AppError> {
        let stdout = child
            .take_stdout()
            .ok_or_else(|| AppError::internal("task stdout pipe is unavailable"))?;
        let stderr = child
            .take_stderr()
            .ok_or_else(|| AppError::internal("task stderr pipe is unavailable"))?;
        let failed = Arc::new(AtomicBool::new(false));
        let stdout = spawn_reader(stdout, log.clone(), "stdout", Arc::clone(&failed))?;
        let stderr = match spawn_reader(stderr, log.clone(), "stderr", Arc::clone(&failed)) {
            Ok(stderr) => stderr,
            Err(error) => {
                drop(stdout);
                return Err(error);
            }
        };
        Ok(Self {
            stdout,
            stderr,
            failed,
        })
    }

    pub(super) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    pub(super) fn finish(self) -> Result<(), AppError> {
        join(self.stdout, "stdout")?;
        join(self.stderr, "stderr")
    }
}

fn spawn_reader(
    reader: impl Read + Send + 'static,
    log: TaskLog,
    label: &'static str,
    failed: Arc<AtomicBool>,
) -> Result<JoinHandle<io::Result<()>>, AppError> {
    thread::Builder::new()
        .name(format!("zcheck-{label}"))
        .spawn(move || {
            let result = copy(reader, &log, label);
            if result.is_err() {
                failed.store(true, Ordering::Release);
            }
            result
        })
        .map_err(|error| AppError::internal(format!("cannot start {label} reader: {error}")))
}

fn copy(mut reader: impl Read, log: &TaskLog, label: &str) -> io::Result<()> {
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        log.write_stream(label, &buffer[..read])?;
    }
}

fn join(handle: JoinHandle<io::Result<()>>, label: &str) -> Result<(), AppError> {
    handle
        .join()
        .map_err(|_| AppError::internal(format!("{label} reader panicked")))?
        .map_err(|error| AppError::internal(format!("cannot capture task {label}: {error}")))
}
