use std::io;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::time::Duration;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

use crate::IpcError;

const READ_TIMEOUT: Duration = Duration::from_secs(5);

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

pub(crate) struct PipeRead {
    file: File,
}

impl PipeRead {
    pub fn from_fd_guard(fd_guard: FdGuard) -> Self {
        let file = unsafe { File::from_raw_fd(fd_guard.into_raw_fd()) };
        Self { file }
    }

    pub async fn read_frame(&mut self) -> Result<Vec<u8>, IpcError> {
        let read_future = async {
            let mut header = [0u8; 4];
            self.file
                .read_exact(&mut header)
                .await
                .map_err(|e| IpcError::Fd4ReadFailed {
                    detail: e.to_string(),
                })?;
            let len = u32::from_be_bytes(header);
            let mut payload = vec![0u8; len as usize];
            self.file
                .read_exact(&mut payload)
                .await
                .map_err(|e| IpcError::Fd4ReadFailed {
                    detail: e.to_string(),
                })?;
            Ok(payload)
        };

        match timeout(READ_TIMEOUT, read_future).await {
            Ok(Ok(payload)) => Ok(payload),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(IpcError::Timeout {
                elapsed_ms: READ_TIMEOUT.as_millis() as u64,
                stdout_bytes: vec![],
                stdout_truncated: false,
                stderr_bytes: vec![],
                stderr_truncated: false,
            }),
        }
    }
}

pub(crate) struct PipeWriter {
    file: File,
}

impl PipeWriter {
    pub fn from_fd_guard(fd_guard: FdGuard) -> Self {
        let std_file = unsafe { std::fs::File::from_raw_fd(fd_guard.into_raw_fd()) };
        let file = File::from_std(std_file);
        Self { file }
    }

    pub async fn write_frame(&mut self, payload: &[u8]) -> Result<(), IpcError> {
        let len = u32::try_from(payload.len()).map_err(|_| {
            IpcError::PayloadTooLarge(u32::try_from(payload.len()).unwrap_or(u32::MAX))
        })?;
        self.file
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| IpcError::Fd3WriteFailed { detail: e.to_string() })?;
        self.file
            .write_all(payload)
            .await
            .map_err(|e| IpcError::Fd3WriteFailed { detail: e.to_string() })?;
        self.file.flush().await.map_err(|e| IpcError::Fd3WriteFailed { detail: e.to_string() })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_frame_times_out_when_writer_blocks() {
        let guard = PipeGuard::new().unwrap();
        let (read_fd, write_fd) = guard.into_parts();
        let mut reader = PipeRead::from_fd_guard(read_fd);
        drop(write_fd);

        let result = reader.read_frame().await;
        assert!(matches!(
            result,
            Err(IpcError::Timeout {
                elapsed_ms: 5000,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn read_frame_returns_data_when_writer_sends_frame() {
        let guard = PipeGuard::new().unwrap();
        let (read_fd, write_fd) = guard.into_parts();
        let mut reader = PipeRead::from_fd_guard(read_fd);
        let write_file = unsafe { std::fs::File::from_raw_fd(write_fd.into_raw_fd()) };
        let mut writer = File::from_std(write_file);

        let payload = b"hello world".to_vec();
        let len_bytes = (payload.len() as u32).to_be_bytes();
        writer.write_all(&len_bytes).await.unwrap();
        writer.write_all(&payload).await.unwrap();
        drop(writer);

        let result = reader.read_frame().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), payload);
    }

    #[tokio::test]
    async fn pipe_writer_reads_128kb_payload_perfect_roundtrip() {
        let guard = PipeGuard::new().unwrap();
        let (read_fd, write_fd) = guard.into_parts();
        let mut reader = PipeRead::from_fd_guard(read_fd);
        let mut writer = PipeWriter::from_fd_guard(write_fd);

        let payload: Vec<u8> = (0..128 * 1024).map(|i| (i % 256) as u8).collect();
        let payload_clone = payload.clone();
        let write_handle = tokio::spawn(async move {
            writer.write_frame(&payload_clone).await.unwrap();
            drop(writer);
        });

        let result = reader.read_frame().await;
        write_handle.await.unwrap();
        assert!(result.is_ok(), "read_frame failed: {:?}", result);
        let received = result.unwrap();
        assert_eq!(received.len(), 128 * 1024, "payload length mismatch");
        assert_eq!(received, payload, "payload content mismatch");
    }

    #[tokio::test]
    async fn pipe_writer_reads_exactly_64kb_payload() {
        let guard = PipeGuard::new().unwrap();
        let (read_fd, write_fd) = guard.into_parts();
        let mut reader = PipeRead::from_fd_guard(read_fd);
        let mut writer = PipeWriter::from_fd_guard(write_fd);

        let payload: Vec<u8> = (0..64 * 1024).map(|i| (i % 256) as u8).collect();
        let payload_clone = payload.clone();
        let write_handle = tokio::spawn(async move {
            writer.write_frame(&payload_clone).await.unwrap();
            drop(writer);
        });

        let result = reader.read_frame().await;
        write_handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), payload);
    }

    #[tokio::test]
    async fn pipe_writer_reads_64kb_plus_one_byte_payload() {
        let guard = PipeGuard::new().unwrap();
        let (read_fd, write_fd) = guard.into_parts();
        let mut reader = PipeRead::from_fd_guard(read_fd);
        let mut writer = PipeWriter::from_fd_guard(write_fd);

        let payload: Vec<u8> = (0..(64 * 1024 + 1)).map(|i| (i % 256) as u8).collect();
        let payload_clone = payload.clone();
        let write_handle = tokio::spawn(async move {
            writer.write_frame(&payload_clone).await.unwrap();
            drop(writer);
        });

        let result = reader.read_frame().await;
        write_handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), payload);
    }

    #[tokio::test]
    async fn pipe_writer_reads_empty_payload() {
        let guard = PipeGuard::new().unwrap();
        let (read_fd, write_fd) = guard.into_parts();
        let mut reader = PipeRead::from_fd_guard(read_fd);
        let mut writer = PipeWriter::from_fd_guard(write_fd);

        let payload: Vec<u8> = vec![];
        writer.write_frame(&payload).await.unwrap();
        drop(writer);

        let result = reader.read_frame().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), payload);
    }
}
