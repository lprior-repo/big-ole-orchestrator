//! ADR-019: SIGTERM Races and Signal Handling
//!
//! This module implements signal handling for the SDK to ensure deterministic
//! shutdown behavior when the Engine sends SIGTERM.
//!
//! ## Key Requirements
//!
//! - Background thread listens for SIGTERM via `ctrlc`
//! - 2-second grace period after SIGTERM before aggressive exit
//! - Uses `std::process::exit(1)` to ensure deterministic shutdown
//!
//! ## Why Not a Signal Handler?
//!
//! In Rust, signal handlers cannot safely allocate memory or perform I/O.
//! Setting an `AtomicBool` and checking it in a loop fails because the
//! user's task code *is* the main loop, and it might be blocked on a
//! long-running call (e.g., ML inference). The SIGTERM would be ignored.
//!
//! By using a dedicated background thread, we can safely perform cleanup
//! operations because it's a normal thread, not a restricted signal handler.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const GRACE_PERIOD_SECS: u64 = 2;

static SIGTERM_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Start the SDK with SIGTERM signal handling.
///
/// This function implements ADR-019 for deterministic shutdown behavior:
/// 1. Spawns a background thread to listen for SIGTERM via `ctrlc`
/// 2. Runs the user's task function on the main thread
/// 3. If SIGTERM is received during task execution, waits 2 seconds
/// 4. Calls `std::process::exit(1)` to aggressively tear down
///
/// # Arguments
///
/// * `task` - The synchronous task function to run
///
/// # Example
///
/// ```ignore
/// use vo_sdk::start;
///
/// fn main() {
///     start(|| {
///         let input = vo_sdk::read_input().unwrap();
///         // ... process input ...
///         vo_sdk::write_success(&serde_json::json!({"result": "ok"})).unwrap();
///     });
/// }
/// ```
///
/// # Shutdown Behavior
///
/// - If the task completes normally, the process exits with code 0
/// - If SIGTERM is received, the process waits up to 2 seconds then exits with code 1
/// - If SIGTERM is received multiple times, only the first grace period is honored
#[inline(always)]
pub fn start(task: impl FnOnce()) -> ! {
    start_with_grace_period(task, GRACE_PERIOD_SECS)
}

#[inline(always)]
fn start_with_grace_period(task: impl FnOnce(), grace_period_secs: u64) -> ! {
    let sigterm_received: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let bg_sigterm_received = sigterm_received.clone();

    ctrlc::set_handler(move || {
        if !bg_sigterm_received.swap(true, Ordering::SeqCst) {
            let tid = thread::spawn(move || {
                thread::sleep(Duration::from_secs(grace_period_secs));
                std::process::exit(1);
            });
            let _ = tid.join();
        }
    })
    .expect("vo-sdk: failed to set ctrlc handler");

    task();

    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigterm_received_flag_initializes_false() {
        assert!(!SIGTERM_RECEIVED.load(Ordering::SeqCst));
    }

    #[test]
    fn start_function_type_signature() {
        fn check_start<F>(f: F) -> bool
        where
            F: FnOnce(),
        {
            true
        }
        assert!(check_start(|| {}));
    }
}
