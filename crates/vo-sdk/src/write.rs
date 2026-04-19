//! FD4 write output logic.
//!
//! ## Write-once invariant
//! The `is_written` guard is set **before** any I/O attempt.
//! Even if the write fails, subsequent calls return `SdkError::AlreadyWritten`.

use std::io::Write;
use std::os::unix::io::FromRawFd;

use serde_json::Value;

use crate::{SdkError, TaskFailureKind};

static IS_WRITTEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024;
const MAX_MESSAGE_BYTES: usize = 1024;

fn is_fd_valid(fd: std::os::unix::io::RawFd) -> bool {
    let borrowed = unsafe { std::os::unix::io::BorrowedFd::borrow_raw(fd) };
    borrowed.try_clone_to_owned().is_ok()
}

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

/// Write a success result to FD4.
///
/// # Errors
/// Returns `SdkError` if already written or write fails.
pub fn write_success(output: &Value) -> Result<(), SdkError> {
    if IS_WRITTEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(SdkError::AlreadyWritten);
    }
    if !is_fd_valid(4) {
        return Err(SdkError::WriteError);
    }
    // SAFETY: FD4 is defined by contract as the write output stream.
    let mut fd4 = unsafe { std::fs::File::from_raw_fd(4) };
    let mut is_written_dummy = false;
    write_success_inner(&mut fd4, output, &mut is_written_dummy)
}

pub(crate) fn write_success_inner<W: Write>(
    writer: &mut W,
    output: &Value,
    is_written: &mut bool,
) -> Result<(), SdkError> {
    if *is_written {
        return Err(SdkError::AlreadyWritten);
    }
    // Set guard BEFORE I/O — write-once invariant holds even on failure.
    *is_written = true;

    let bytes = serde_json::to_vec(&SuccessEnvelope {
        status: "success",
        output,
    })
    .map_err(|_| SdkError::WriteError)?;

    if bytes.len() > MAX_OUTPUT_SIZE {
        return Err(SdkError::WriteError);
    }

    writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
}

/// Write a failure result to FD4.
///
/// # Errors
/// Returns `SdkError` if already written, input invalid, or write fails.
pub fn write_failure(kind: TaskFailureKind, message: &str) -> Result<(), SdkError> {
    if IS_WRITTEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(SdkError::AlreadyWritten);
    }
    if !is_fd_valid(4) {
        return Err(SdkError::WriteError);
    }
    // SAFETY: FD4 is defined by contract as the write output stream.
    let mut fd4 = unsafe { std::fs::File::from_raw_fd(4) };
    let mut is_written_dummy = false;
    write_failure_inner(&mut fd4, kind, message, &mut is_written_dummy)
}

pub(crate) fn write_failure_inner<W: Write>(
    writer: &mut W,
    kind: TaskFailureKind,
    message: &str,
    is_written: &mut bool,
) -> Result<(), SdkError> {
    if *is_written {
        return Err(SdkError::AlreadyWritten);
    }
    // Set guard BEFORE I/O — write-once invariant holds even on failure.
    *is_written = true;

    // Message limit is enforced in bytes (see crate-level docs).
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(SdkError::InvalidInput);
    }

    let bytes = serde_json::to_vec(&FailureEnvelope {
        status: "failure",
        kind: kind.as_str(),
        message,
    })
    .map_err(|_| SdkError::WriteError)?;

    writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
}
