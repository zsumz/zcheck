//! Command-line application composition.

mod args;
mod discovery;
mod doctor;
mod error;
mod execution;
mod exit;
#[cfg(windows)]
mod internal_host;
mod render;
mod run;

#[cfg(windows)]
pub(crate) use internal_host::run_if_requested;
pub(crate) use run::run;
