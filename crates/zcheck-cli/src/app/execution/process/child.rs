//! Cross-platform task-tree containment and liveness.

#[cfg(target_os = "linux")]
use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::process::{ChildStderr, ChildStdout, Command, ExitStatus, Stdio};

use command_group::{CommandGroup, GroupChild};
#[cfg(unix)]
use command_group::{Signal, UnixChildExt};
#[cfg(unix)]
use nix::errno::Errno;
#[cfg(unix)]
use nix::sys::signal::killpg;
#[cfg(unix)]
use nix::unistd::Pid;
#[cfg(windows)]
use win32job::Job;
use zcheck_core::Task;

#[derive(Debug)]
pub(super) struct ManagedChild {
    inner: GroupChild,
    leader: Option<ExitStatus>,
    #[cfg(windows)]
    observer: Job,
}

impl ManagedChild {
    pub(super) fn spawn(
        program: &Path,
        cwd: &Path,
        task: &Task,
        arguments: &[String],
    ) -> io::Result<Self> {
        #[cfg(unix)]
        let inner = spawn_unix(program, cwd, task, arguments)?;
        #[cfg(windows)]
        let (inner, observer) = spawn_windows(program, cwd, task, arguments)?;
        Ok(Self {
            inner,
            leader: None,
            #[cfg(windows)]
            observer,
        })
    }

    pub(super) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.inner.inner().stdout.take()
    }

    pub(super) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.inner.inner().stderr.take()
    }

    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.leader.is_none() {
            self.leader = self.inner.inner().try_wait()?;
        }
        if self.tree_is_alive()? {
            return Ok(None);
        }
        if self.leader.is_none() {
            self.leader = Some(self.inner.inner().wait()?);
        }
        Ok(self.leader)
    }

    #[cfg(unix)]
    pub(super) fn request_graceful_stop(&self) -> io::Result<bool> {
        match self.inner.signal(Signal::SIGTERM) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == ErrorKind::InvalidInput => Ok(true),
            Err(error) => Err(error),
        }
    }

    #[cfg(windows)]
    pub(super) const fn request_graceful_stop() -> bool {
        false
    }

    pub(super) fn force_stop(&mut self) -> io::Result<()> {
        match self.inner.kill() {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::InvalidInput => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[cfg(unix)]
    fn tree_is_alive(&self) -> io::Result<bool> {
        let raw = i32::try_from(self.inner.id())
            .map_err(|_| io::Error::other("process group ID is too large"))?;
        match killpg(Pid::from_raw(raw), None) {
            Ok(()) | Err(Errno::EPERM) => {
                #[cfg(target_os = "linux")]
                if self.leader.is_some() {
                    return linux_group_has_live_member(raw);
                }
                Ok(true)
            }
            Err(Errno::ESRCH) => Ok(false),
            Err(error) => Err(io::Error::from_raw_os_error(error as i32)),
        }
    }

    #[cfg(windows)]
    fn tree_is_alive(&self) -> io::Result<bool> {
        self.observer
            .query_process_id_list()
            .map(|members| !members.is_empty())
            .map_err(io::Error::other)
    }
}

#[cfg(target_os = "linux")]
fn linux_group_has_live_member(group: i32) -> io::Result<bool> {
    for entry in fs::read_dir("/proc")? {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if process_state_and_group(&stat)
            .is_some_and(|(state, candidate)| candidate == group && state != b'Z')
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(target_os = "linux")]
fn process_state_and_group(stat: &str) -> Option<(u8, i32)> {
    let (_, fields) = stat.rsplit_once(") ")?;
    let mut fields = fields.split_ascii_whitespace();
    let state = fields.next()?.as_bytes();
    if state.len() != 1 {
        return None;
    }
    fields.next()?;
    let group = fields.next()?.parse::<i32>().ok()?;
    Some((state[0], group))
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if self.try_wait().ok().flatten().is_none() {
            let _ = self.force_stop();
        }
    }
}

#[cfg(unix)]
fn command(program: &Path, cwd: &Path, task: &Task, arguments: &[String]) -> Command {
    let mut command = Command::new(program);
    configure(&mut command, cwd, task);
    command.args(&arguments[1..]);
    command
}

fn configure(command: &mut Command, cwd: &Path, task: &Task) {
    command
        .current_dir(cwd)
        .envs(task.env())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
}

#[cfg(unix)]
fn spawn_unix(
    program: &Path,
    cwd: &Path,
    task: &Task,
    arguments: &[String],
) -> io::Result<GroupChild> {
    command(program, cwd, task, arguments).group_spawn()
}

#[cfg(windows)]
fn spawn_windows(
    program: &Path,
    cwd: &Path,
    task: &Task,
    arguments: &[String],
) -> io::Result<(GroupChild, Job)> {
    use std::io::Write;
    use std::os::windows::io::AsRawHandle;

    let observer = Job::create().map_err(io::Error::other)?;
    let host = std::env::current_exe()?;
    let mut command = Command::new(host);
    configure(&mut command, cwd, task);
    command
        .arg("__zcheck-task-host")
        .arg(program)
        .args(&arguments[1..])
        .stdin(Stdio::piped());
    let mut group = command.group();
    group.kill_on_drop(true);
    let mut child = group.spawn()?;
    let assigned = observer.assign_process(child.inner().as_raw_handle() as isize);
    if let Err(error) = assigned {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::other(error));
    }
    let released = child
        .inner()
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("internal task host has no containment gate"))?;
    let mut released = released;
    released.write_all(&[1])?;
    drop(released);
    Ok((child, observer))
}

#[cfg(test)]
#[path = "child_test.rs"]
mod child_test;
