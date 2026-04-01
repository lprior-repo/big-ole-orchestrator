use vo_ipc::error::{IpcError, ConfigError};
use std::io;
use std::path::PathBuf;

#[test]
fn ipc_error_config_display() {
    let err = IpcError::Config(ConfigError::TimeoutMustBePositive { timeout_ms: 0 });
    assert_eq!(format!("{}", err), "timeout must be greater than 0ms, got 0");
}

#[test]
fn ipc_error_pipe_setup_failed_display() {
    let err = IpcError::PipeSetupFailed { detail: "oops".to_string() };
    assert_eq!(format!("{}", err), "failed to create subprocess pipes: oops");
}

#[test]
fn ipc_error_spawn_failed_display() {
    let err = IpcError::SpawnFailed { detail: "oops".to_string() };
    assert_eq!(format!("{}", err), "failed to spawn subprocess: oops");
}

#[test]
fn ipc_error_wait_failed_display() {
    let err = IpcError::WaitFailed { detail: "oops".to_string() };
    assert_eq!(format!("{}", err), "failed to wait for subprocess: oops");
}

#[test]
fn ipc_error_fd4_read_failed_display() {
    let err = IpcError::Fd4ReadFailed { detail: "oops".to_string() };
    assert_eq!(format!("{}", err), "failed to read fd4 payload: oops");
}

#[test]
fn ipc_error_stderr_read_failed_display() {
    let err = IpcError::StderrReadFailed { detail: "oops".to_string() };
    assert_eq!(format!("{}", err), "failed to capture stderr: oops");
}

#[test]
fn ipc_error_signal_failed_display() {
    let err = IpcError::SignalFailed { detail: "oops".to_string() };
    assert_eq!(format!("{}", err), "failed to signal subprocess: oops");
}

#[test]
fn ipc_error_io_error_display() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "oops");
    let err = IpcError::IoError(io_err);
    assert_eq!(format!("{}", err), "IO error: oops");
}

#[test]
fn ipc_error_fd3_write_failed_display() {
    let err = IpcError::Fd3WriteFailed { detail: "broken pipe".to_string() };
    assert_eq!(format!("{}", err), "failed to write fd3 payload: broken pipe");
}

#[test]
fn config_error_variants() {
    let err1 = ConfigError::ProgramMissing { path: PathBuf::from("/missing") };
    assert_eq!(format!("{}", err1), "program path does not exist: \"/missing\"");
    
    let err2 = ConfigError::ProgramNotExecutable { path: PathBuf::from("/root") };
    assert_eq!(format!("{}", err2), "program path is not executable: \"/root\"");
}
