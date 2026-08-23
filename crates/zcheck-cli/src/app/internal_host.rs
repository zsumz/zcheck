//! Windows-only containment gate for race-free nested Job Object assignment.

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Stdio};

use super::exit;

const COMMAND: &str = "__zcheck-task-host";

pub(crate) fn run_if_requested() -> Option<i32> {
    let mut arguments = std::env::args_os();
    arguments.next()?;
    if arguments.next().as_deref() != Some(OsStr::new(COMMAND)) {
        return None;
    }
    Some(run(arguments))
}

fn run(mut arguments: impl Iterator<Item = std::ffi::OsString>) -> i32 {
    let mut gate = [0_u8; 1];
    if std::io::stdin().read_exact(&mut gate).is_err() || gate != [1] {
        eprintln!("zcheck internal task host was not assigned to its Job Object");
        return exit::INTERNAL;
    }
    let Some(program) = arguments.next() else {
        eprintln!("zcheck internal task host received no executable");
        return exit::INTERNAL;
    };
    match Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .status()
    {
        Ok(status) => status.code().unwrap_or(exit::QUALIFICATION_FAILED),
        Err(error) => {
            eprintln!("zcheck internal task launch failed: {error}");
            exit::INTERNAL
        }
    }
}
