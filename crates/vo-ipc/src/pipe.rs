use std::io;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::IpcError;

pub(crate) struct FdGuard(RawFd);

impl FdGuard {
    pub const fn new(fd: RawFd) -> Self {
        Self(fd)
    }

    pub const fn fd(&self) -> RawFd {
        self.0
    }

    /// Consumes the guard and returns the raw file descriptor.
    /// The caller now owns the FD and must ensure it is properly closed.
    pub fn into_raw_fd(self) -> RawFd {
        let fd = self.0;
        std::mem::forget(self);
        fd
    }
}

impl Drop for FdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

impl AsRawFd for FdGuard {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl FromRawFd for FdGuard {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self(fd)
    }
}

impl IntoRawFd for FdGuard {
    fn into_raw_fd(self) -> RawFd {
        self.into_raw_fd()
    }
}

pub(crate) struct PipeGuard {
    read_fd: FdGuard,
    write_fd: FdGuard,
}

impl PipeGuard {
    pub fn new() -> Result<Self, IpcError> {
        let mut fds = [0; 2];
        let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        if res != 0 {
            return Err(IpcError::PipeSetupFailed {
                detail: io::Error::last_os_error().to_string(),
            });
        }
        Ok(Self {
            read_fd: FdGuard::new(fds[0]),
            write_fd: FdGuard::new(fds[1]),
        })
    }

    pub fn read_fd(&self) -> RawFd {
        self.read_fd.fd()
    }

    pub fn write_fd(&self) -> RawFd {
        self.write_fd.fd()
    }

    pub fn into_parts(self) -> (FdGuard, FdGuard) {
        (self.read_fd, self.write_fd)
    }

    pub fn read_fd_into_file(self) -> std::fs::File {
        // SAFETY: We transfer ownership of the FD to the File.
        // The FdGuard's Drop will not run since we use into_raw_fd.
        unsafe { std::fs::File::from_raw_fd(self.read_fd.into_raw_fd()) }
    }

    pub fn write_fd_into_file(self) -> std::fs::File {
        // SAFETY: We transfer ownership of the FD to the File.
        // The FdGuard's Drop will not run since we use into_raw_fd.
        unsafe { std::fs::File::from_raw_fd(self.write_fd.into_raw_fd()) }
    }
}

impl Default for PipeGuard {
    fn default() -> Self {
        Self::new().expect("pipe2 failed")
    }
}

pub(crate) fn create_pipe() -> Result<PipeGuard, IpcError> {
    PipeGuard::new()
}
