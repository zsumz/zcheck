//! Task-graph execution composition.

mod artifacts;
mod cancellation;
mod evidence;
mod github;
mod logs;
mod model;
mod preflight;
mod process;
mod receipt_io;
mod render;
mod reporter;
mod repository;
mod result;
mod scheduler;
mod selection;
mod task;

pub(crate) use artifacts::ArtifactOptions;
pub(crate) use model::ExecutionOptions;
pub(crate) use preflight::{blocked_tasks, find_tool};
pub(crate) use render::render;
pub(crate) use repository::inspect_repository_state;
pub(crate) use scheduler::execute;
