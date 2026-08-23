//! Process-wide Ctrl-C observation with invocation-scoped state.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use signal_hook::SigId;
use signal_hook::consts::SIGINT;

use super::super::error::AppError;

#[derive(Debug)]
pub(super) struct Cancellation {
    requested: Arc<AtomicBool>,
    registration: SigId,
}

impl Cancellation {
    pub(super) fn install() -> Result<Self, AppError> {
        let requested = Arc::new(AtomicBool::new(false));
        let registration =
            signal_hook::flag::register(SIGINT, Arc::clone(&requested)).map_err(|error| {
                AppError::internal(format!("cannot install Ctrl-C handler: {error}"))
            })?;
        Ok(Self {
            requested,
            registration,
        })
    }

    pub(super) fn requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    pub(super) fn request(&self) {
        self.requested.store(true, Ordering::SeqCst);
    }
}

impl Drop for Cancellation {
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.registration);
    }
}
