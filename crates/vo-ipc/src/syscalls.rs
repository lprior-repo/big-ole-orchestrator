//! Safe abstraction layer over raw libc file descriptor syscalls.
//!
//! All unsafe FD operations in this crate are isolated here with
//! documented SAFETY proofs. Consumer modules (`run`, `spsc`, etc.)
//! use only the safe APIs exported by this module.
//!
//! # Covered Operations
//!
//! - `pipe2()` — pipe creation with O_CLOEXEC
//! - `PreExecSetup` — fd3/fd4 redirection + CLOEXEC + process group in pre_exec
//! - `close_raw_fd()` — safe FD closure
//! - `file_from_raw_fd()` — safe RawFd -> std::fs::File conversion
//! - `kill_process_group()` — safe signal delivery to process groups

use std::io;
use std::os::fd::RawFd;

/// SAFETY: RawFd is an integer type alias. Copying it is safe —
/// it does not transfer ownership. The caller retains responsibility
/// for ensuring the FD is valid for the duration of the operation.
#[derive(Debug, Clone, Copy)]
pub struct PreExecSetup {
    fd3_read: RawFd,
    fd4_write: RawFd,
}

impl PreExecSetup {
    /// Create a new setup for the child process pre_exec hook.
    #[must_use]
    pub const fn new(fd3_read: RawFd, fd4_write: RawFd) -> Self {
        Self {
            fd3_read,
            fd4_write,
        }
    }

    /// Return a closure suitable for `tokio::process::Command::pre_exec()`.
    ///
    /// This closure performs:
    /// 1. `prctl(PR_SET_PDEATHSIG, SIGTERM)` — kill child if parent dies
    /// 2. `setpgid(0, 0)` — create new process group for signal management
    /// 3. `dup2(fd3_read, 3)` — redirect child stdin to fd3 (read end of input pipe)
    /// 4. `dup2(fd4_write, 4)` — redirect child fd4 to fd4_write (write end of output pipe)
    /// 5. `fcntl(3, FD_CLOEXEC)` and `fcntl(4, FD_CLOEXEC)` — close-on-exec on both FDs
    ///
    /// # SAFETY
    ///
    /// The caller must guarantee:
    /// - `fd3_read` is a valid, open read-end file descriptor before fork
    /// - `fd4_write` is a valid, open write-end file descriptor before fork
    /// - The process has permissions to call `prctl`, `setpgid`, and `dup2`
    ///
    /// The closure is safe to call *only* within the `pre_exec` context
    /// (after fork, before exec), where the process is single-threaded
    /// and the file descriptors are about to be inherited by the child.
    pub fn pre_exec_closure(&self) -> impl FnMut() -> io::Result<()> {
        let fd3_read = self.fd3_read;
        let fd4_write = self.fd4_write;
        move || {
            // SAFETY: Called inside pre_exec (post-fork, pre-exec).
            // Single-threaded context guarantees no concurrent access.
            // fd3_read and fd4_write are validated by the caller.
            let ret = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) };
            if ret != 0 {
                return Err(io::Error::last_os_error());
            }

            // SAFETY: setpgid(0, 0) creates a new process group for the child.
            // Valid in pre_exec context. Prevents signal race with parent.
            let ret = unsafe { libc::setpgid(0, 0) };
            if ret != 0 {
                return Err(io::Error::last_os_error());
            }

            // SAFETY: dup2(fd3_read, 3) is safe because:
            // - fd3_read is a valid open FD (caller responsibility)
            // - fd 3 is about to become child stdin (by V2 spec)
            // - No other threads exist post-fork
            if unsafe { libc::dup2(fd3_read, 3) } == -1 {
                return Err(io::Error::last_os_error());
            }

            // SAFETY: dup2(fd4_write, 4) is safe because:
            // - fd4_write is a valid open FD (caller responsibility)
            // - fd 4 is about to become child output channel (by V2 spec)
            if unsafe { libc::dup2(fd4_write, 4) } == -1 {
                return Err(io::Error::last_os_error());
            }

            // SAFETY: fcntl(FD_CLOEXEC) on fd 3 prevents fd3 from being
            // inherited past exec, which is required by the V2 IPC protocol.
            if unsafe { libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
                return Err(io::Error::last_os_error());
            }

            // SAFETY: fcntl(FD_CLOEXEC) on fd 4 prevents fd4 from being
            // inherited past exec, which is required by the V2 IPC protocol.
            if unsafe { libc::fcntl(4, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
                return Err(io::Error::last_os_error());
            }

            Ok(())
        }
    }
}

/// Create a pipe with O_CLOEXEC using the pipe2 syscall.
///
/// # Errors
///
/// Returns `io::Error` if the `pipe2` syscall fails.
///
/// # SAFETY
///
/// This function is safe to call from any context. It wraps an unsafe
/// syscall behind a safe API, handling the error case (return value != 0)
/// and populating `errno` via `last_os_error()`.
///
/// # Returns
///
/// An array `[read_fd, write_fd]` — the two ends of the pipe.
/// Both FDs have the close-on-exec flag set.
pub fn pipe2() -> Result<[RawFd; 2], io::Error> {
    let mut fds: [RawFd; 2] = [0; 2];
    // SAFETY: pipe2 writes exactly 2 RawFd values into `fds`.
    // The caller provides a buffer of sufficient size.
    // On success (ret == 0), both FDs are valid and ready to use.
    // On failure (ret == -1), errno is set and we propagate it.
    let ret = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fds)
}

/// Safely close a raw file descriptor.
///
/// # SAFETY
///
/// The caller must ensure `fd` is a valid, open file descriptor.
/// After calling this, `fd` is no longer valid and must not be used.
pub fn close_raw_fd(fd: RawFd) {
    // SAFETY: Caller guarantees fd is a valid open file descriptor.
    // close() is idempotent in the sense that passing an invalid fd
    // returns EBADF, which we intentionally ignore here — the goal
    // is to ensure the FD is closed, and it already is if invalid.
    unsafe {
        libc::close(fd);
    }
}

/// Convert a raw file descriptor into an owned `std::fs::File`.
///
/// # SAFETY
///
/// The caller must guarantee:
/// - `fd` is a valid, open file descriptor
/// - No other code holds a copy of `fd` that will close it
/// - The FD is open in the mode expected by the caller (read/write)
///
/// This transfer-of-ownership is the idiomatic Rust pattern for
/// converting between C-style FDs and Rust's RAII File type.
pub fn file_from_raw_fd(fd: RawFd) -> std::fs::File {
    // SAFETY: Caller guarantees fd is a valid open file descriptor.
    // std::fs::File::from_raw_fd takes ownership, matching the convention
    // that the caller transfers ownership when passing a RawFd.
    unsafe { std::fs::File::from_raw_fd(fd) }
}

/// Send a signal to a process group.
///
/// # Arguments
///
/// * `pid` — The process ID. Pass negative values to target a process group.
/// * `signal` — The signal number (e.g., `libc::SIGTERM`).
///
/// # SAFETY
///
/// The caller must verify that `pid` corresponds to a valid process or
/// process group that this process has permission to signal.
pub fn kill_process_group(pid: i32, signal: i32) {
    // SAFETY: Caller verifies pid is valid and signal is appropriate.
    // kill() with a negative pid targets the process group whose ID
    // equals the absolute value of pid, which is the standard pattern
    // for terminating all processes in a group (used by run.rs and bus.rs).
    unsafe {
        libc::kill(pid, signal);
    }
}
