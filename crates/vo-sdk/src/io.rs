//! I/O helpers: read_input, write_success, write_failure with single-write guard.
//!
//! ## Write-once invariant
//! `write_success` / `write_failure` may be called at most once per process lifetime.
//! The guard is set *before* any I/O attempt — even if the write fails, subsequent
//! calls are rejected with `SdkError::AlreadyWritten`.
//!
//! ## Message limit
//! The failure message limit (1024) is enforced in **bytes**, not characters.
//! A multibyte UTF-8 message may be rejected below 1024 chars if it exceeds 1024 bytes.

use std::io::Read;
use std::io::Write;
use std::os::unix::io::FromRawFd;

use serde_json::Value;
use vo_types::TaskInputEnvelope;
use zeroize::Zeroizing;

use crate::SdkError;
use crate::TaskFailureKind;
use crate::TaskInput;

// ============================================================================
// Single-Write Guard
// ============================================================================

static IS_WRITTEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024;
const MAX_MESSAGE_BYTES: usize = 1024;

/// Check if output has already been written.
///
/// Use this to query the single-write guard state before calling [`write_success`]
/// or [`write_failure`].
///
/// # Example
///
/// ```
/// use vo_sdk::{is_written, write_success};
/// use serde_json::json;
///
/// assert!(!is_written()); // Initially false
/// let _ = write_success(&json!({ "ok": true }));
/// assert!(is_written());  // True after writing
/// ```
#[must_use]
pub fn is_written() -> bool {
    IS_WRITTEN.load(std::sync::atomic::Ordering::SeqCst)
}

/// Write a success result to FD4.
///
/// This function may only be called once per process lifetime due to the
/// single-write guard. Subsequent calls return [`SdkError::AlreadyWritten`].
///
/// # Example (in a task binary)
///
/// ```ignore
/// use vo_sdk::{write_success, read_input, execute_node};
/// use serde_json::json;
///
/// // After executing a node:
/// let result = json!({ "receipt_id": "abc123", "amount": 99.99 });
/// write_success(&result).expect("failed to write success");
/// ```
///
/// # Errors
///
/// Returns `SdkError::AlreadyWritten` if called a second time.
/// Returns `SdkError::FdNotOpen` if FD4 is not open.
/// Returns `SdkError::WriteError` if the write fails.
pub fn write_success(output: &Value) -> Result<(), SdkError> {
    if IS_WRITTEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(SdkError::AlreadyWritten);
    }
    if !is_fd_valid(4) {
        return Err(SdkError::WriteError);
    }
    // SAFETY: FD4 is defined by contract as the write output stream.
    let mut fd4 = unsafe { std::fs::File::from_raw_fd(4) };
    write_success_inner(&mut fd4, output)
}

pub(crate) fn write_success_inner<W: Write>(
    writer: &mut W,
    output: &Value,
) -> Result<(), SdkError> {
    let bytes = serde_json::to_vec(&SuccessEnvelope {
        status: "success",
        output,
    })
    .map_err(|_| SdkError::WriteError)?;

    let len = bytes.len();
    if len > MAX_OUTPUT_SIZE {
        return Err(SdkError::WriteError);
    }

    let len_u32 = len as u32;
    writer
        .write_all(&len_u32.to_be_bytes())
        .map_err(|_| SdkError::WriteError)?;
    writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
}

/// Internal variant of `write_success` that accepts an explicit `is_written` state parameter.
/// Used by tests to verify guard behavior with in-memory writers.
pub fn write_success_inner_with_state<W: Write>(
    writer: &mut W,
    output: &Value,
    is_written: &mut bool,
) -> Result<(), SdkError> {
    if *is_written {
        return Err(SdkError::AlreadyWritten);
    }
    *is_written = true;

    let bytes = serde_json::to_vec(&SuccessEnvelope {
        status: "success",
        output,
    })
    .map_err(|_| SdkError::WriteError)?;

    let len = bytes.len();
    if len > MAX_OUTPUT_SIZE {
        return Err(SdkError::WriteError);
    }

    let len_u32 = len as u32;
    writer
        .write_all(&len_u32.to_be_bytes())
        .map_err(|_| SdkError::WriteError)?;
    writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
}

/// Write a failure result to FD4.
///
/// # Message Size Limit
/// The message is limited to 1024 **bytes**, not characters. A 4-byte UTF-8 character
/// (e.g., a Chinese character) counts as 4 bytes toward this limit. An ASCII character
/// counts as 1 byte. For example, `"中文中文中文"` is 10 characters but 20 bytes.
///
/// # Errors
///
/// Returns `SdkError::AlreadyWritten` if called a second time.
/// Returns `SdkError::InvalidInput` if the message exceeds 1024 bytes.
/// Returns `SdkError::FdNotOpen` if FD4 is not open.
/// Returns `SdkError::WriteError` if the write fails.
pub fn write_failure(kind: TaskFailureKind, message: &str) -> Result<(), SdkError> {
    if IS_WRITTEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(SdkError::AlreadyWritten);
    }
    if !is_fd_valid(4) {
        return Err(SdkError::WriteError);
    }
    // SAFETY: FD4 is defined by contract as the write output stream.
    let mut fd4 = unsafe { std::fs::File::from_raw_fd(4) };
    write_failure_inner(&mut fd4, kind, message)
}

pub(crate) fn write_failure_inner<W: Write>(
    writer: &mut W,
    kind: TaskFailureKind,
    message: &str,
) -> Result<(), SdkError> {
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(SdkError::InvalidInput);
    }

    let bytes = serde_json::to_vec(&FailureEnvelope {
        status: "failure",
        kind: kind.as_str(),
        message,
    })
    .map_err(|_| SdkError::WriteError)?;

    let len_u32 = bytes.len() as u32;
    writer
        .write_all(&len_u32.to_be_bytes())
        .map_err(|_| SdkError::WriteError)?;
    writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
}

/// Internal variant of `write_failure` that accepts an explicit `is_written` state parameter.
/// Used by tests to verify guard behavior with in-memory writers.
pub fn write_failure_inner_with_state<W: Write>(
    writer: &mut W,
    kind: TaskFailureKind,
    message: &str,
    is_written: &mut bool,
) -> Result<(), SdkError> {
    if *is_written {
        return Err(SdkError::AlreadyWritten);
    }
    *is_written = true;

    if message.len() > MAX_MESSAGE_BYTES {
        return Err(SdkError::InvalidInput);
    }

    let bytes = serde_json::to_vec(&FailureEnvelope {
        status: "failure",
        kind: kind.as_str(),
        message,
    })
    .map_err(|_| SdkError::WriteError)?;

    let len_u32 = bytes.len() as u32;
    writer
        .write_all(&len_u32.to_be_bytes())
        .map_err(|_| SdkError::WriteError)?;
    writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
}

// ============================================================================
// Input Reading
// ============================================================================

static IS_READ: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const MAX_INPUT_SIZE: usize = 10 * 1024 * 1024;

/// Check if input has already been read.
///
/// # Example
///
/// ```
/// use vo_sdk::is_read;
///
/// assert!(!is_read()); // Initially false
/// // After read_input() is called, this returns true
/// ```
#[must_use]
pub fn is_read() -> bool {
    IS_READ.load(std::sync::atomic::Ordering::SeqCst)
}

/// Read task input from FD3.
///
/// The input is expected to be a JSON envelope containing task data and secrets.
/// This function may only be called once per process lifetime.
///
/// # Example (in a task binary)
///
/// ```ignore
/// use vo_sdk::{read_input, write_success};
/// use serde_json::json;
///
/// let input = read_input().expect("failed to read input");
///
/// // Access the input data
/// let data = input.data();
/// let secret = input.secret("API_KEY");
///
/// // Process and write result
/// let result = json!({ "processed": true });
/// write_success(&result).expect("failed to write success");
/// ```
///
/// # Errors
///
/// Returns `SdkError::FdNotOpen` if FD3 is not open or already read.
/// Returns `SdkError::InvalidInput` if the input is malformed or exceeds size limits.
pub fn read_input() -> Result<TaskInput, SdkError> {
    if IS_READ.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(SdkError::FdNotOpen);
    }
    if !is_fd_valid(3) {
        return Err(SdkError::FdNotOpen);
    }
    // SAFETY: FD3 is defined by contract as the read input stream.
    let mut fd3 = unsafe { std::fs::File::from_raw_fd(3) };
    read_input_inner(&mut fd3)
}

pub(crate) fn read_input_inner<R: Read>(reader: &mut R) -> Result<TaskInput, SdkError> {
    let mut buf = Zeroizing::new(Vec::new());
    let len = reader
        .take((MAX_INPUT_SIZE + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|_| SdkError::FdNotOpen)?;

    if len == 0 {
        return Err(SdkError::InvalidInput);
    }
    if len > MAX_INPUT_SIZE {
        return Err(SdkError::InvalidInput);
    }

    parse_envelope(&buf)
}

/// Internal variant of `read_input` that accepts an explicit `is_read` state parameter.
/// Used by tests to verify guard behavior with in-memory readers.
///
/// # Errors
/// Returns `SdkError::FdNotOpen` if already read or I/O fails.
pub fn read_input_inner_with_state<R: Read>(
    reader: &mut R,
    is_read: &mut bool,
) -> Result<TaskInput, SdkError> {
    if *is_read {
        return Err(SdkError::FdNotOpen);
    }
    *is_read = true;

    let mut buf = Zeroizing::new(Vec::new());
    let len = reader
        .take((MAX_INPUT_SIZE + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|_| SdkError::FdNotOpen)?;

    if len == 0 {
        return Err(SdkError::InvalidInput);
    }
    if len > MAX_INPUT_SIZE {
        return Err(SdkError::InvalidInput);
    }

    parse_envelope(&buf)
}

/// Internal variant of `read_input` that uses an atomic guard for concurrent coordination.
/// Used by concurrent tests to verify exactly-one semantics.
///
/// # Errors
/// Returns `SdkError::FdNotOpen` if already read or I/O fails.
pub fn read_input_inner_with_atomic_guard<R: Read>(
    reader: &mut R,
    guard: &std::sync::atomic::AtomicBool,
) -> Result<TaskInput, SdkError> {
    if guard
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        return Err(SdkError::FdNotOpen);
    }

    let mut buf = Zeroizing::new(Vec::new());
    let len = reader
        .take((MAX_INPUT_SIZE + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|_| SdkError::FdNotOpen)?;

    if len == 0 {
        return Err(SdkError::InvalidInput);
    }
    if len > MAX_INPUT_SIZE {
        return Err(SdkError::InvalidInput);
    }

    parse_envelope(&buf)
}

// ============================================================================
// Secret Access (ADR-014: In-Memory Secret Vault)
// ============================================================================

/// Read a secret from the FD3 payload by key.
///
/// This is a convenience wrapper around `read_input()` that directly returns
/// the secret value for the given key. Secrets are never passed as environment
/// variables — they travel only through the in-memory FD3 pipe (ADR-014).
///
/// # Errors
/// Returns `SdkError` if FD3 is not open, already read, input is invalid,
/// or the key is not present in the secrets map.
pub fn secret(key: &str) -> Result<String, SdkError> {
    let input = read_input()?;
    input.secret(key).cloned().ok_or(SdkError::InvalidInput)
}

// ============================================================================
// Helper Functions
// ============================================================================

fn is_fd_valid(fd: std::os::unix::io::RawFd) -> bool {
    let borrowed = unsafe { std::os::unix::io::BorrowedFd::borrow_raw(fd) };
    borrowed.try_clone_to_owned().is_ok()
}

/// Parse and validate a JSON buffer into a `TaskInput`.
fn parse_envelope(buf: &[u8]) -> Result<TaskInput, SdkError> {
    let json = std::str::from_utf8(buf).map_err(|_| SdkError::InvalidInput)?;
    let env: TaskInputEnvelope = serde_json::from_str(json).map_err(|_| SdkError::InvalidInput)?;
    env.parse().ok_or(SdkError::InvalidInput)
}

// ============================================================================
// Envelope Types
// ============================================================================

#[derive(serde::Serialize)]
struct SuccessEnvelope<'a> {
    status: &'a str,
    output: &'a Value,
}

#[derive(serde::Serialize)]
struct FailureEnvelope<'a> {
    status: &'a str,
    kind: &'a str,
    message: &'a str,
}
