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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;

    #[test]
    fn create_pipe_returns_valid_fds() {
        let (read_fd, write_fd) = create_pipe().expect("pipe2 should succeed");
        assert_ne!(read_fd, write_fd);
        assert!(read_fd >= 0);
        assert!(write_fd >= 0);
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn create_pipe_fds_are_cloexec() {
        let (read_fd, write_fd) = create_pipe().expect("pipe2 should succeed");
        let read_flags = unsafe { libc::fcntl(read_fd, libc::F_GETFD) };
        let write_flags = unsafe { libc::fcntl(write_fd, libc::F_GETFD) };
        assert_eq!(read_flags & libc::FD_CLOEXEC, libc::FD_CLOEXEC);
        assert_eq!(write_flags & libc::FD_CLOEXEC, libc::FD_CLOEXEC);
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn create_pipe_write_then_read() {
        let (read_fd, write_fd) = create_pipe().expect("pipe2 should succeed");
        let msg = b"hello pipe";
        let written = unsafe { libc::write(write_fd, msg.as_ptr() as *const _, msg.len()) };
        assert_eq!(written as usize, msg.len());
        unsafe { libc::close(write_fd); }

        let mut buf = [0u8; 16];
        let n = unsafe { libc::read(read_fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        assert_eq!(n as usize, msg.len());
        assert_eq!(&buf[..msg.len()], msg);
        unsafe { libc::close(read_fd); }
    }

    #[test]
    fn create_pipe_multiple_sequential_pipes() {
        let pipe1 = create_pipe().expect("first pipe should succeed");
        let pipe2 = create_pipe().expect("second pipe should succeed");
        let pipe3 = create_pipe().expect("third pipe should succeed");

        assert_ne!(pipe1.0, pipe2.0);
        assert_ne!(pipe2.0, pipe3.0);

        unsafe {
            libc::close(pipe1.0);
            libc::close(pipe1.1);
            libc::close(pipe2.0);
            libc::close(pipe2.1);
            libc::close(pipe3.0);
            libc::close(pipe3.1);
        }
    }

    #[test]
    fn create_pipe_read_returns_zero_on_write_end_closed() {
        let (read_fd, write_fd) = create_pipe().expect("pipe2 should succeed");
        unsafe { libc::close(write_fd); }

        let mut buf = [0u8; 16];
        let n = unsafe { libc::read(read_fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        assert_eq!(n, 0);
        unsafe { libc::close(read_fd); }
    }
}
