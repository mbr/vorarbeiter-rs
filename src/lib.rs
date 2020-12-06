//! A supervisor for processes that allow clean shutdowns.
//!
//! See [`Supervisor`] for the core functionality. Real applications will likely want to use
//! [`setup_term_flag`] as well.
//!
//! # Example
//!
//! ```no_run
//! use std::process;
//!
//! // The default kill timeout is 10 seconds, which is fine here.
//! let mut supervisor = vorarbeiter::Supervisor::default();
//!
//! // Spawns three new child processes and adds them to the supervisor.
//! for _ in 0..3 {
//!     let child = process::Command::new("my-subcommand").spawn().unwrap();
//!     supervisor.add_child(child);
//! }
//!
//! // Terminate all child processes, waiting for each to be completed or killed.
//! drop(supervisor);
//! ```

use nix::sys::signal;
use nix::unistd;
use std::{io, process, sync, thread, time};

/// A supervisor for child processes.
///
/// Supports default, which will result in a `kill_timeout` of 10 seconds.
///
/// When the supervisor is dropped, it will kill all of its owned child processes using
/// [`shutdown_process`] in the reverse order they were added, ignoring any errors.
#[derive(Debug)]
pub struct Supervisor {
    /// Supervised child processes.
    children: Vec<process::Child>,
    /// How long to wait before sending SIGKILL after SIGTERM.
    kill_timeout: time::Duration,
    /// Time between checks if process has terminated.
    poll_interval: time::Duration,
}

impl Supervisor {
    /// Adds a child process to the supervisor.
    pub fn add_child(&mut self, child: process::Child) {
        self.children.push(child)
    }
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
///
/// First sends a `SIGTERM` to the child process and polls it for completion every `poll_interval`.
/// If the process does not finish within `kill_timeout`, sends a `SIGKILL`.
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

/// Sets up a termination flag.
///
/// A pure convenience function, creates an atomic boolean that is initially false, but will be set
/// to `true` should the process receive a `SIGINT`, `SIGTERM` or `SIGQUIT`. This works around the
/// issue that receiving any of these signals would by default not result in any `Drop`
/// implementations to be called.
///
/// # Example
///
/// ```rust
/// # use std::sync;
/// let term = vorarbeiter::setup_term_flag().unwrap();
///
/// while !term.load(sync::atomic::Ordering::Relaxed) {
/// # break;
/// // Main loop code here.
/// }
/// ```
pub fn setup_term_flag() -> Result<sync::Arc<sync::atomic::AtomicBool>, io::Error> {
    let term = sync::Arc::new(sync::atomic::AtomicBool::new(false));

    // Ensure that all signals call exit, we need to execute `Drop` properly.
    for &signal in &[
        signal_hook::SIGINT,
        signal_hook::SIGTERM,
        signal_hook::SIGQUIT,
    ] {
        signal_hook::flag::register(signal, term.clone())?;
    }

    Ok(term)
}
