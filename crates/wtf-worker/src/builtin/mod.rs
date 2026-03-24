//! Built-in activity handlers for the Worker (bead wtf-qgum).
//!
//! Provides `register_defaults()` which registers two handlers:
//! - `"echo"` — returns `task.payload` unchanged (identity function).
//! - `"sleep"` — parses `{"ms": u64}` from payload, sleeps cooperatively via
//!   `tokio::time::sleep`, then returns `"slept"`.
//!
//! # Usage
//! ```ignore
//! let mut worker = Worker::new(js, "builtin-worker", None);
//! wtf_worker::register_defaults(&mut worker);
//! worker.run(shutdown_rx).await?;
//! ```

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use bytes::Bytes;
use tokio::time;

use crate::queue::ActivityTask;
use crate::worker::Worker;

/// Error message returned by [`sleep_handler`] for any parse failure.
const SLEEP_PARSE_ERR: &str = "sleep handler: invalid payload: expected {\"ms\": <u64>}";

/// Return value bytes for a successful sleep: the JSON string `"slept"`.
const SLEPT_RESULT: Bytes = Bytes::from_static(b"\"slept\"");

/// Register built-in activity handlers on a `Worker`.
///
/// Registers `"echo"` (returns payload unchanged) and `"sleep"` (parses
/// `{"ms": u64}`, sleeps, returns `"slept"`).
///
/// Idempotent — calling twice overwrites with identical handlers.
/// Pre-existing handlers for other activity types are preserved.
pub fn register_defaults(worker: &mut Worker) {
    worker.register("echo", echo_handler);
    worker.register("sleep", sleep_handler);
}

/// Returns `task.payload` as-is. Never fails.
///
/// The handler moves `task.payload` into the return value (zero-copy on the
/// `Bytes` inner buffer).
///
/// # Errors
/// This handler never returns an error.
pub async fn echo_handler(task: ActivityTask) -> Result<Bytes, String> {
    Ok(task.payload)
}

/// Parses `{"ms": u64}` from `task.payload`, sleeps for that many
/// milliseconds via `tokio::time::sleep` (cooperative), then returns
/// `Ok(Bytes::from_static(b"\"slept\""))`.
///
/// # Errors
/// Returns `Err("sleep handler: invalid payload: expected {\"ms\": <u64>}")` if:
/// - payload is not valid UTF-8
/// - payload is not valid JSON
/// - JSON root is not an object
/// - object does not contain a `"ms"` field with a `u64` value
pub async fn sleep_handler(task: ActivityTask) -> Result<Bytes, String> {
    let ms = parse_sleep_ms(&task.payload)?;
    time::sleep(Duration::from_millis(ms)).await;
    Ok(SLEPT_RESULT.clone())
}

/// Pure calculation: parse the `"ms"` field from a JSON payload.
///
/// Returns `Ok(ms)` if the payload is a valid JSON object containing a `"ms"`
/// key with a `u64` value. Returns `Err` with a descriptive message otherwise.
fn parse_sleep_ms(payload: &[u8]) -> Result<u64, String> {
    let text = std::str::from_utf8(payload)
        .map_err(|_| SLEEP_PARSE_ERR.to_owned())?;

    let value = serde_json::from_str::<serde_json::Value>(text)
        .map_err(|_| SLEEP_PARSE_ERR.to_owned())?;

    value
        .as_object()
        .and_then(|obj| obj.get("ms"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| SLEEP_PARSE_ERR.to_owned())
}

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests_echo;

#[cfg(test)]
mod tests_sleep;
