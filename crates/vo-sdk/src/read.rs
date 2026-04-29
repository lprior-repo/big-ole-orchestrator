//! FD3 read input logic.

use std::io::Read;
use std::os::unix::io::FromRawFd;

use vo_types::{TaskInput, TaskInputEnvelope};
use zeroize::Zeroizing;

use crate::SdkError;

static IS_READ: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn is_fd_valid(fd: std::os::unix::io::RawFd) -> bool {
    let borrowed = unsafe { std::os::unix::io::BorrowedFd::borrow_raw(fd) };
    borrowed.try_clone_to_owned().is_ok()
}

/// Read task input from FD3.
///
/// # Errors
/// Returns `SdkError` if FD is not open, already read, or input is invalid.
pub fn read_input() -> Result<TaskInput, SdkError> {
    if !is_fd_valid(3) {
        return Err(SdkError::FdNotOpen);
    }
    // SAFETY: FD3 is defined by contract as the read input stream.
    let mut fd3 = unsafe { std::fs::File::from_raw_fd(3) };
    let mut is_read_dummy = false;
    read_input_inner(&mut fd3, &mut is_read_dummy)
}

const MAX_INPUT_SIZE: usize = 10 * 1024 * 1024;

/// Parse and validate a JSON buffer into a `TaskInput`.
fn parse_envelope(buf: &[u8]) -> Result<TaskInput, SdkError> {
    let json = std::str::from_utf8(buf).map_err(|_| SdkError::InvalidInput)?;
    let env: TaskInputEnvelope =
        serde_json::from_str(json).map_err(|_| SdkError::InvalidInput)?;
    env.parse().ok_or(SdkError::InvalidInput)
}

pub(crate) fn read_input_inner<R: Read>(
    reader: &mut R,
    _is_read: &mut bool,
) -> Result<TaskInput, SdkError> {
    if IS_READ
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
