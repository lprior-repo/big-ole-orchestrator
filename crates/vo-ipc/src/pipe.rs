use std::io;
use std::os::fd::RawFd;

use crate::IpcError;

pub(crate) fn create_pipe() -> Result<(RawFd, RawFd), IpcError> {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if res != 0 {
        return Err(IpcError::PipeSetupFailed {
            detail: io::Error::last_os_error().to_string(),
        });
    }
    Ok(fds.into())
}
