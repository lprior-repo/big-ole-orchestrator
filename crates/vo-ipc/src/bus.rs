use crate::config::SubprocessConfig;
use crate::envelope::{Fd3Envelope, Fd4Envelope};
use crate::error::IpcError;
use crate::run::SubprocessOutput;
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, OwnedPermit};
use tokio::sync::watch;
use tokio::time::{timeout, Duration};

const DEFAULT_BACKPRESSURE_LIMIT: usize = 64;
const BUS_TIMEOUT_MS: u64 = 5000;
const DEFAULT_BACKPRESSURE_THRESHOLD: f32 = 0.80;

#[derive(Debug, Clone)]
pub struct BusConfig {
    backpressure_limit: usize,
    backpressure_threshold: f32,
    timeout_ms: u64,
}

impl BusConfig {
    #[must_use]
    pub const fn new(backpressure_limit: usize, timeout_ms: u64) -> Self {
        Self {
            backpressure_limit,
            backpressure_threshold: DEFAULT_BACKPRESSURE_THRESHOLD,
            timeout_ms,
        }
    }

    #[must_use]
    pub const fn with_backpressure_threshold(mut self, threshold: f32) -> Self {
        self.backpressure_threshold = threshold;
        self
    }

    #[must_use]
    pub const fn backpressure_limit(&self) -> usize {
        self.backpressure_limit
    }

    #[must_use]
    pub const fn backpressure_threshold(&self) -> f32 {
        self.backpressure_threshold
    }

    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            backpressure_limit: DEFAULT_BACKPRESSURE_LIMIT,
            backpressure_threshold: DEFAULT_BACKPRESSURE_THRESHOLD,
            timeout_ms: BUS_TIMEOUT_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusMessage {
    Request(Fd3Envelope),
    Response(Fd4Envelope),
    Drained,
}

#[derive(Debug)]
pub struct MessageBus {
    child: tokio::process::Child,
    fd3_write: Option<tokio::fs::File>,
    fd4_read: Option<tokio::fs::File>,
    config: BusConfig,
    sender: mpsc::Sender<BusMessage>,
    receiver: mpsc::Receiver<BusMessage>,
    stderr_reader: Option<tokio::process::ChildStderr>,
    stdout_reader: Option<tokio::process::ChildStdout>,
    backpressure_tx: watch::Sender<bool>,
    backpressure_rx: watch::Receiver<bool>,
}

impl MessageBus {
    #[allow(clippy::unused_async)]
    pub async fn spawn(config: SubprocessConfig, bus_config: BusConfig) -> Result<Self, IpcError> {
        let (fd3_read, fd3_write) = create_pipe()?;
        let (fd4_read, fd4_write) = create_pipe()?;

        let mut command = tokio::process::Command::new(config.executable_path());
        command.args(config.argv());
        command.env_clear();
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        unsafe {
            command.pre_exec(move || {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(fd3_read, 3) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(fd4_write, 4) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                // Close the original pipe FDs in the child (they've been dup'd to 3 and 4)
                libc::close(fd3_read);
                libc::close(fd4_write);
                Ok(())
            });
        }

        let mut child = command.spawn().map_err(|e| IpcError::SpawnFailed {
            detail: e.to_string(),
        })?;

        unsafe {
            libc::close(fd3_read);
            libc::close(fd4_write);
        }

        let fd3_writer =
            unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd3_write)) };
        let fd4_reader = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd4_read)) };
        let stderr_reader = child
            .stderr
            .take()
            .ok_or_else(|| IpcError::StderrReadFailed {
                detail: "Failed to take stderr".to_string(),
            })?;

        let (sender, receiver) = mpsc::channel(bus_config.backpressure_limit);
        let (backpressure_tx, backpressure_rx) = watch::channel(false);

        Ok(Self {
            child,
            fd3_write: Some(fd3_writer),
            fd4_read: Some(fd4_reader),
            config: bus_config,
            sender,
            receiver,
            stderr_reader: Some(stderr_reader),
            stdout_reader: None,
            backpressure_tx,
            backpressure_rx,
        })
    }

    pub async fn send(&mut self, envelope: Fd3Envelope) -> Result<(), BusError> {
        self.update_backpressure();
        let permit = self
            .sender
            .reserve()
            .await
            .map_err(|_| BusError::BusClosed)?;
        permit.send(BusMessage::Request(envelope));
        Ok(())
    }

    #[allow(clippy::unused_async)]
    pub async fn send_with_permit(
        permit: OwnedPermit<BusMessage>,
        envelope: Fd3Envelope,
        backpressure_rx: &mut watch::Receiver<bool>,
    ) -> Result<(), BusError> {
        if *backpressure_rx.borrow() {
            let timeout_duration = Duration::from_secs(30);
            let result = timeout(timeout_duration, async {
                while *backpressure_rx.borrow() {
                    if backpressure_rx.changed().await.is_err() {
                        return;
                    }
                }
            })
            .await;

            if result.is_err() || *backpressure_rx.borrow() {
                return Err(BusError::BackpressureTimeout);
            }
        }
        permit.send(BusMessage::Request(envelope));
        Ok(())
    }

    fn update_backpressure(&self) {
        let capacity_ratio = self.sender.capacity() as f32 / self.max_capacity() as f32;
        let should_backpressure = capacity_ratio > (1.0 - self.config.backpressure_threshold);
        if should_backpressure != *self.backpressure_rx.borrow() {
            let _ = self.backpressure_tx.send(should_backpressure);
        }
    }

    #[must_use]
    pub fn backpressure_rx(&self) -> watch::Receiver<bool> {
        self.backpressure_rx.clone()
    }

    pub async fn recv(&mut self) -> Result<BusMessage, BusError> {
        self.receiver.recv().await.ok_or(BusError::BusClosed)
    }

    pub fn try_recv(&mut self) -> Result<BusMessage, BusError> {
        self.receiver.try_recv().map_err(|_| BusError::BusClosed)
    }

    pub fn capacity(&self) -> usize {
        self.sender.capacity()
    }

    pub const fn max_capacity(&self) -> usize {
        self.config.backpressure_limit
    }

    pub fn is_full(&self) -> bool {
        self.sender.capacity() == 0
    }

    pub async fn drain(mut self) -> Result<SubprocessOutput, IpcError> {
        drop(self.sender);

        let mut fd3_write = self.fd3_write.take();
        if let Some(ref mut writer) = fd3_write {
            writer
                .shutdown()
                .await
                .map_err(|e| IpcError::Fd3WriteFailed {
                    detail: e.to_string(),
                })?;
        }

        let stderr_task = tokio::task::spawn(crate::stderr::read_bounded_stderr(
            self.stderr_reader.take().ok_or(BusError::AlreadyConsumed)?,
        ));

        let stdout_reader =
            self.stdout_reader
                .take()
                .ok_or_else(|| IpcError::StdoutReadFailed {
                    detail: "stdout reader not available".to_string(),
                })?;
        let stdout_task = tokio::task::spawn(crate::stderr::read_bounded_stderr(stdout_reader));

        let mut fd4_read = self.fd4_read.take();

        let read_task = async {
            let mut reader = fd4_read.take().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "IPC reader already consumed")
            })?;
            let mut total_read = 0;
            let mut header = [0u8; 4];
            while total_read < 4 {
                let n = reader.read(&mut header[total_read..]).await?;
                if n == 0 {
                    if total_read == 0 {
                        return Ok::<Vec<u8>, std::io::Error>(vec![]);
                    }
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "early eof",
                    ));
                }
                total_read += n;
            }
            let len = u32::from_be_bytes(header);
            if len > crate::envelope::MAX_PAYLOAD_SIZE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("fd4 payload too large: {len} bytes"),
                ));
            }
            let mut bytes = vec![0u8; len as usize];
            reader.read_exact(&mut bytes).await?;
            Ok::<Vec<u8>, std::io::Error>(bytes)
        };

        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        let fd4_bytes = match timeout(timeout_duration, read_task).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(e)) => {
                return Err(IpcError::Fd4ReadFailed {
                    detail: e.to_string(),
                });
            }
            Err(_) => {
                return Err(IpcError::Timeout {
                    elapsed_ms: self.config.timeout_ms,
                    stdout_bytes: vec![],
                    stdout_truncated: false,
                    stderr_bytes: vec![],
                    stderr_truncated: false,
                });
            }
        };

        let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
            detail: e.to_string(),
        })?;
        let stdout_res = stdout_task.await.map_err(|e| IpcError::StdoutReadFailed {
            detail: e.to_string(),
        })?;
        let stderr_capture = stderr_res.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to capture stderr during drain");
            crate::stderr::StderrCapture::empty()
        });
        let stdout_capture = stdout_res.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to capture stdout during drain");
            crate::stderr::StderrCapture::empty()
        });

        let exit_status = self.child.wait().await.map_err(|e| IpcError::WaitFailed {
            detail: e.to_string(),
        })?;

        if exit_status.success() {
            Ok(SubprocessOutput {
                fd4_bytes,
                stdout_bytes: stdout_capture.bytes,
                stdout_truncated: stdout_capture.truncated,
                stderr_bytes: stderr_capture.bytes,
                stderr_truncated: stderr_capture.truncated,
            })
        } else {
            Err(IpcError::ProcessFailed {
                exit_code: map_exit_code(exit_status),
                stdout_bytes: stdout_capture.bytes,
                stdout_truncated: stdout_capture.truncated,
                stderr_bytes: stderr_capture.bytes,
                stderr_truncated: stderr_capture.truncated,
            })
        }
    }

    pub async fn shutdown(self) -> Result<(), IpcError> {
        let _ = self.drain().await;
        Ok(())
    }
}

fn create_pipe() -> Result<(RawFd, RawFd), IpcError> {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if res != 0 {
        return Err(IpcError::PipeSetupFailed {
            detail: std::io::Error::last_os_error().to_string(),
        });
    }
    Ok(fds.into())
}

#[must_use]
pub(crate) fn map_exit_code(status: std::process::ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| status.signal().map_or(-1, |s| 128 + s))
}

#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("bus is closed")]
    BusClosed,
    #[error("backpressure limit reached")]
    BackpressureLimitReached,
    #[error("backpressure timeout: child process fell behind")]
    BackpressureTimeout,
    #[error("timeout")]
    Timeout,
    #[error("IPC reader already consumed")]
    AlreadyConsumed,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<mpsc::error::SendError<BusMessage>> for BusError {
    fn from(_: mpsc::error::SendError<BusMessage>) -> Self {
        Self::BusClosed
    }
}

impl From<BusError> for IpcError {
    fn from(err: BusError) -> Self {
        match err {
            BusError::BusClosed => IpcError::ProcessFailed {
                exit_code: -1,
                stdout_bytes: vec![],
                stdout_truncated: false,
                stderr_bytes: vec![],
                stderr_truncated: false,
            },
            BusError::BackpressureLimitReached => IpcError::ProcessFailed {
                exit_code: -1,
                stdout_bytes: vec![],
                stdout_truncated: false,
                stderr_bytes: vec![],
                stderr_truncated: false,
            },
            BusError::BackpressureTimeout => IpcError::BackpressureTimeout { wait_seconds: 30 },
            BusError::Timeout => IpcError::Timeout {
                elapsed_ms: 0,
                stdout_bytes: vec![],
                stdout_truncated: false,
                stderr_bytes: vec![],
                stderr_truncated: false,
            },
            BusError::AlreadyConsumed => IpcError::AlreadyConsumed,
            BusError::IoError(e) => IpcError::IoError(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_config_default_backpressure() {
        let config = BusConfig::default();
        assert_eq!(config.backpressure_limit, DEFAULT_BACKPRESSURE_LIMIT);
        assert_eq!(config.timeout_ms, BUS_TIMEOUT_MS);
    }

    #[test]
    fn bus_config_custom_values() {
        let config = BusConfig::new(128, 10000);
        assert_eq!(config.backpressure_limit, 128);
        assert_eq!(config.timeout_ms, 10000);
    }

    #[test]
    fn bus_error_display() {
        assert_eq!(BusError::BusClosed.to_string(), "bus is closed");
        assert_eq!(
            BusError::BackpressureLimitReached.to_string(),
            "backpressure limit reached"
        );
        assert_eq!(BusError::Timeout.to_string(), "timeout");
    }

    #[tokio::test]
    async fn bus_config_setters_getters() {
        let config = BusConfig::default();
        assert_eq!(config.backpressure_limit(), DEFAULT_BACKPRESSURE_LIMIT);
        assert_eq!(config.timeout_ms(), BUS_TIMEOUT_MS);
    }
}
