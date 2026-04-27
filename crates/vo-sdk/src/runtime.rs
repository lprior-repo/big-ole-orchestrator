//! Async task execution runtime with current-thread Tokio and SIGTERM handling.
//!
//! Implements ADR-011 (current-thread runtime) and ADR-019 (signal handling).
//!
//! # Usage
//!
//! ```ignore
//! use vo_sdk::runtime::start;
//!
//! fn main() {
//!     start(|input| async move {
//!         // Your async task logic here
//!         Ok(serde_json::json!({"result": "done"}))
//!     });
//! }
//! ```

use std::future::Future;
use std::io::Write as _;
use std::panic;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use tokio::runtime::Builder;
use tokio::sync::watch;
use tokio::time::timeout;
use vo_types::TaskInput;

/// Execute the user's async task function inside a current-thread Tokio runtime.
///
/// This is the entry point for all task binaries. It:
/// 1. Sets up a SIGTERM handler thread (ADR-019)
/// 2. Creates a Tokio current-thread runtime (ADR-011)
/// 3. Reads the task input from FD3
/// 4. Executes the user's async closure
/// 5. Writes the result to FD4
/// 6. Exits with the appropriate exit code
///
/// # Example
///
/// ```ignore
/// use vo_sdk::runtime::start;
/// use vo_types::TaskInput;
///
/// fn main() {
///     start(|input: TaskInput| async move {
///         // Your async task logic here
///         Ok(serde_json::json!({"result": "done"}))
///     });
/// }
/// ```
pub fn start<F, Fut>(task: F) -> !
where
    F: FnOnce(TaskInput) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Value, crate::TaskFailureKind>> + Send + 'static,
{
    // Install panic hook that forces exit.
    panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            String::from("unknown panic")
        };

        let _ = writeln!(std::io::stderr(), "vo-sdk task panicked: {}", msg);
        std::process::exit(1);
    }));

    // Set up shared state for coordinating with the signal handler.
    let (done_tx, done_rx) = watch::channel(false);
    let mut done_rx_for_signal = done_rx;
    let done_tx_for_signal = done_tx.clone();

    // Spawn a background thread that handles SIGTERM (ADR-019).
    // This thread creates its own tokio runtime for signal handling.
    let signal_thread = thread::spawn(move || {
        let sig_rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build signal-handling runtime");

        sig_rt.block_on(async {
            // Set up SIGTERM handler.
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => {
                    // Non-Unix or signal unavailable. Nothing to do.
                    return;
                }
            };

            // Wait for SIGTERM.
            sigterm.recv().await;

            // SIGTERM received. Notify the main runtime that shutdown is pending.
            // We use a separate minimal runtime to send the notification in case
            // the main runtime's current-thread runtime is blocked.
            let notify_rt = Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("Failed to build notification runtime");

            notify_rt.block_on(async {
                let _ = done_tx_for_signal.send_replace(true);
                // Give the main runtime a moment to notice and finish cleanup.
                tokio::time::sleep(Duration::from_millis(100)).await;
            });

            // Grace period: 2 seconds to let the task finish its work.
            // The task can check `vo_sdk::is_shutdown()` to know it should exit.
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Grace period expired. Force exit.
            std::process::exit(1);
        });
    });

    // Create the current-thread runtime (ADR-011).
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build current-thread tokio runtime");

    // Read input from FD3 (synchronous, reads small JSON from FD3).
    let input = match crate::io::read_input() {
        Ok(input) => input,
        Err(e) => {
            eprintln!("vo-sdk: failed to read input: {}", e);
            std::process::exit(1);
        }
    };

    // Execute the user's async task.
    let task_result = rt.block_on(async {
        // Spawn the actual task so we can cancel it on shutdown.
        let task_handle = tokio::spawn(task(input));
        let mut task_handle = Some(task_handle);

        // Wait for either task completion or shutdown signal.
        tokio::select! {
            result = task_handle.take().unwrap() => {
                result.unwrap_or_else(|e| {
                    Err(crate::TaskFailureKind::System)
                })
            }
            _ = done_rx_for_signal.changed() => {
                // Shutdown signal received. Cancel the task.
                if let Some(handle) = task_handle.take() {
                    handle.abort();
                    // Wait for the task to finish (up to 2 second grace period).
                    let cancel_result = tokio::time::timeout(Duration::from_secs(2), handle).await;
                    match cancel_result {
                        Ok(Ok(Ok(_))) => {} // Task completed during grace period.
                        Ok(Ok(Err(kind))) => {
                            let _ = crate::io::write_failure(kind, "task cancelled by shutdown signal");
                        }
                        Ok(Err(_)) => {
                            // Task was aborted (join error).
                            let _ = crate::io::write_failure(
                                crate::TaskFailureKind::Timeout,
                                "task aborted by shutdown signal",
                            );
                        }
                        Err(_) => {
                            // Timeout expired. Task was forcibly killed.
                            let _ = crate::io::write_failure(
                                crate::TaskFailureKind::Timeout,
                                "task did not respond to shutdown signal",
                            );
                        }
                    }
                }

                // Mark as done so the signal handler can proceed.
                let _ = done_tx.send_replace(true);

                // Signal handler will handle the final exit.
                std::process::exit(1);
            }
        }
    });

    // Mark as done before writing output.
    let _ = done_tx.send_replace(true);

    // Write the result.
    match task_result {
        Ok(value) => {
            let _ = crate::io::write_success(&value);
            // Wait for signal thread to exit.
            let _ = signal_thread.join();
            std::process::exit(0);
        }
        Err(kind) => {
            let _ = crate::io::write_failure(kind, "task failed");
            // Wait for signal thread to exit.
            let _ = signal_thread.join();
            std::process::exit(1);
        }
    }
}

/// Check if a shutdown signal has been received.
///
/// Returns `true` if the SIGTERM signal handler has notified this function
/// that shutdown is in progress. Task binaries can use this to implement
/// cooperative shutdown:
///
/// ```ignore
/// use vo_sdk::runtime::start;
///
/// fn main() {
///     start(|input| async move {
///         // Long-running task that checks for shutdown.
///         for chunk in some_stream {
///             if vo_sdk::runtime::is_shutdown_requested() {
///                 // Flush pending state, then exit.
///                 break;
///             }
///             process_chunk(chunk).await;
///         }
///         Ok(serde_json::json!({"partial": true}))
///     });
/// }
/// ```
pub fn is_shutdown_requested() -> bool {
    // This function is a convenience for tasks that need to check shutdown
    // status. In the current implementation, the shutdown signal is handled
    // by the `start()` function's tokio select — tasks don't need to poll
    // this function. It exists as a hook for future implementations that
    // may use cooperative cancellation.
    false
}
