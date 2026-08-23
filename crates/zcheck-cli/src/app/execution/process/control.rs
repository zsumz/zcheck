//! Timeout and interruption control for complete task process trees.

use std::io;
use std::process::ExitStatus;
use std::thread;
use std::time::{Duration, Instant};

use zcheck_core::{DurationSpec, Termination, TerminationReason};

use super::super::super::error::AppError;
use super::super::cancellation::Cancellation;
use super::capture::Readers;
use super::child::ManagedChild;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const TERMINATION_GRACE: Duration = Duration::from_millis(500);

pub(super) struct Completion {
    pub(super) status: ExitStatus,
    pub(super) termination: Option<Termination>,
}

pub(super) fn wait(
    child: &mut ManagedChild,
    started: Instant,
    timeout: Option<&DurationSpec>,
    cancellation: &Cancellation,
    readers: &Readers,
) -> Result<Completion, AppError> {
    let timeout = timeout.map(|value| Duration::from_millis(value.milliseconds()));
    loop {
        if let Some(status) = child.try_wait().map_err(|error| control_error(&error))? {
            return Ok(Completion {
                status,
                termination: None,
            });
        }
        if readers.failed() {
            return Err(AppError::internal(
                "task output capture failed before process-tree completion",
            ));
        }
        let reason = if cancellation.requested() {
            Some(TerminationReason::Interrupted)
        } else if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            Some(TerminationReason::Timeout)
        } else {
            None
        };
        if let Some(reason) = reason {
            let (status, forced) = terminate(child)?;
            return Ok(Completion {
                status,
                termination: Some(Termination { reason, forced }),
            });
        }
        thread::sleep(poll_duration(started, timeout));
    }
}

fn poll_duration(started: Instant, timeout: Option<Duration>) -> Duration {
    timeout.map_or(POLL_INTERVAL, |limit| {
        POLL_INTERVAL.min(limit.saturating_sub(started.elapsed()))
    })
}

fn terminate(child: &mut ManagedChild) -> Result<(ExitStatus, bool), AppError> {
    #[cfg(unix)]
    let graceful = child
        .request_graceful_stop()
        .map_err(|error| control_error(&error))?;
    #[cfg(windows)]
    let graceful = ManagedChild::request_graceful_stop();
    if !graceful {
        return force_and_wait(child).map(|status| (status, true));
    }
    let started = Instant::now();
    while started.elapsed() < TERMINATION_GRACE {
        if let Some(status) = child.try_wait().map_err(|error| control_error(&error))? {
            return Ok((status, false));
        }
        thread::sleep(POLL_INTERVAL);
    }
    force_and_wait(child).map(|status| (status, true))
}

pub(super) fn force_and_wait(child: &mut ManagedChild) -> Result<ExitStatus, AppError> {
    child.force_stop().map_err(|error| control_error(&error))?;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| control_error(&error))? {
            return Ok(status);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn control_error(error: &io::Error) -> AppError {
    AppError::internal(format!("cannot control task process group: {error}"))
}
