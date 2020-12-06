//! A supervisor for processes that allow clean shutdowns.

use nix::sys::signal;
use nix::unistd;
use std::{io, process, thread, time};

/// A supervisor for child processes.
///
/// Supports default, which will result in a `kill_timeout` of 10 seconds.
///
/// When the supervisor is dropped, it will kill all of its owned child processes using
/// `shutdown_process` in the reverse order they were added, ignoring any errors.
#[derive(Debug)]
pub struct Supervisor {
    /// Supervised child processes.
    children: Vec<process::Child>,
    /// How long to wait before sending SIGKILL after SIGTERM.
    kill_timeout: time::Duration,
    /// Time between checks if process has terminated.
    poll_interval: time::Duration,
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        for child in self.children.iter_mut().rev() {
            let _ = shutdown_process(child, self.kill_timeout, self.poll_interval);
        }
    }
}

impl Supervisor {
    /// Create a new supervisor with the given kill timeout.
    pub fn new(kill_timeout: time::Duration) -> Self {
        Supervisor {
            children: Vec::new(),
            kill_timeout,
            poll_interval: time::Duration::from_millis(100),
        }
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Supervisor::new(time::Duration::from_secs(10))
    }
}

/// Shuts down a process using SIGTERM, sending SIGKILL after `timeout`.
pub fn shutdown_process(
    child: &mut process::Child,
    kill_timeout: time::Duration,
    poll_interval: time::Duration,
) -> io::Result<process::ExitStatus> {
    let start = time::Instant::now();
    let pid = unistd::Pid::from_raw(child.id() as i32);

    // Ask nicely via sigterm first.
    signal::kill(pid, signal::Signal::SIGTERM)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    while time::Instant::now() - start < kill_timeout {
        if let Some(exit_status) = child.try_wait()? {
            return Ok(exit_status);
        }

        thread::sleep(poll_interval);
    }

    // If that fails, kill with SIGKILL.
    signal::kill(pid, signal::Signal::SIGKILL)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(child.wait()?)
}
