//! SIGTERM signal handling per ADR-019.
//!
//! ## Overview
//!
//! When the Engine's timeout fires, it sends `SIGTERM` to the child binary. The child
//! is expected to flush state and exit cleanly. However, in Rust, signal handlers cannot
//! safely allocate memory or perform I/O.
//!
//! This module solves the problem by using a dedicated background `std::thread` to
//! intercept OS signals via the `ctrlc` crate. Because `ctrlc` runs signal handlers on
//! a separate OS thread (not the restricted signal handler context), it can safely
//! perform SDK-level cleanup operations.
//!
//! ## The API
//!
//! [`start()`] wraps the user's task function:
//! 1. Spawns a background thread to listen for SIGTERM via `ctrlc`
//! 2. Runs the user's task on the main thread
//! 3. If SIGTERM is received during the task, a 2-second countdown begins
//! 4. After the task completes (or is aborted), calls `std::process::exit(1)` if SIGTERM
//!    was received
//!
//! This ensures deterministic, reliable shutdown behavior that doesn't rely on the
//! developer writing cooperative loop-checking code.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// The grace period duration after SIGTERM before forced exit.
const GRACE_PERIOD: Duration = Duration::from_secs(2);

/// Global flag tracking whether SIGTERM was received.
/// Shared between the signal handler and `sigterm_received()` callers.
static SIGTERM_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Run the user's task function with SIGTERM handling.
///
/// This is the primary entry point for vo-sdk task binaries. It:
/// 1. Sets up signal interception on a background thread
/// 2. Executes the user's task on the main thread
/// 3. If SIGTERM was received during execution, forces exit after the grace period
///
/// The task function runs to completion unless SIGTERM is received and the grace period
/// expires. The background signal thread will call `std::process::exit(1)` after
/// [`GRACE_PERIOD`] if the main thread is still running.
///
/// # Example
///
/// ```ignore
/// use vo_sdk::signal::start;
///
/// fn main() {
///     start(|| {
///         // User's task code here
///         let input = vo_sdk::read_input()?;
///         // ... process ...
///         vo_sdk::write_success(&result)?;
///         Ok(())
///     });
/// }
/// ```
pub fn start<T, F>(task: F) -> T
where
    F: FnOnce() -> T,
{
    let sigterm_received = Arc::new(AtomicBool::new(false));

    // Set up signal handler on a background OS thread via ctrlc.
    // ctrlc installs an OS-level signal handler that routes SIGTERM/SIGINT
    // to a dedicated background thread created by the ctrlc crate.
    ctrlc::set_handler({
        let sigterm_received = Arc::clone(&sigterm_received);
        move || {
            // This runs on the ctrlc background thread, NOT the restricted signal
            // handler context. We can safely allocate, spawn threads, and perform I/O here.

            // Mark that SIGTERM was received (across all threads).
            sigterm_received.store(true, Ordering::SeqCst);
            SIGTERM_RECEIVED.store(true, Ordering::SeqCst);

            // SDK-level cleanup: attempt to flush any pending SDK state.
            // This is where future SDK cleanup logic would go (e.g., flushing
            // pending writes, cleaning up temp files, etc.).

            // After SDK cleanup, enforce the 2-second grace period and force exit.
            // This tears down the main thread regardless of what the user's task
            // is doing — preventing the Engine from having to wait for SIGKILL.
            let _handle = thread::spawn(move || {
                thread::sleep(GRACE_PERIOD);
                std::process::exit(1);
            });
        }
    })
    .expect("failed to register SIGTERM signal handler");

    // Run the user's task on the main thread.
    let result = task();

    // After the task completes, check if SIGTERM was received.
    // If so, the background ctrlc handler has already spawned a timer thread
    // that will call process::exit(1) after GRACE_PERIOD. We exit immediately
    // to avoid any further work while the timer counts down.
    if sigterm_received.load(Ordering::SeqCst) {
        // The timer thread from the signal handler will force exit.
        // We exit immediately here as well to avoid running any more code.
        // The timer thread's exit(1) is the authoritative shutdown path.
        std::process::exit(1);
    }

    result
}

/// Check if a SIGTERM signal was received during the current process lifetime.
///
/// This can be called from within the task function to detect that SIGTERM
/// was received and handle it cooperatively (e.g., break out of a long loop).
///
/// Note: If SIGTERM was received and the grace period expired, `exit(1)` will
/// have already been called by the background signal thread.
#[must_use]
pub fn sigterm_received() -> bool {
    SIGTERM_RECEIVED.load(Ordering::SeqCst)
}
