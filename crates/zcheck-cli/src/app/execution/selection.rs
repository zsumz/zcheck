//! Run-selection validation at the execution boundary.

use zcheck_core::{Plan, PlannedTaskKind};

use super::super::error::AppError;

pub(super) fn validate_passthrough(plan: &Plan, arguments: &[String]) -> Result<(), AppError> {
    if arguments.is_empty() {
        return Ok(());
    }
    if plan.selection().len() != 1 {
        return Err(AppError::usage(
            "process arguments require exactly one selected executable task",
        ));
    }
    let root = plan
        .tasks()
        .iter()
        .find(|task| task.name() == &plan.selection()[0])
        .ok_or_else(|| AppError::internal("selected task is absent from its plan"))?;
    if root.kind() != PlannedTaskKind::Executable {
        return Err(AppError::usage(format!(
            "task `{}` is an aggregate and cannot receive process arguments",
            root.name()
        )));
    }
    Ok(())
}
