use crate::error::ConfigError;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessConfig {
    executable_path: PathBuf,
    timeout_ms: u64,
    fd3_payload: Vec<u8>,
}

impl SubprocessConfig {
    /// Creates a new `SubprocessConfig`.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if:
    /// - `timeout_ms` is zero
    /// - Program path does not exist
    /// - Program path is not a file
    /// - Program path is not executable
    pub fn new<P, B>(path: P, timeout_ms: u64, fd3_payload: B) -> Result<Self, ConfigError>
    where
        P: AsRef<Path>,
        B: Into<Vec<u8>>,
    {
        let p = path.as_ref();
        validate_timeout(timeout_ms)?;
        validate_program_path(p)?;

        let canonical_path = p
            .canonicalize()
            .map_err(|_| ConfigError::ProgramMissing { path: p.to_path_buf() })?;

        Ok(Self {
            executable_path: canonical_path,
            timeout_ms,
            fd3_payload: fd3_payload.into(),
        })
    }

    #[must_use]
    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    #[must_use]
    pub fn fd3_payload(&self) -> &[u8] {
        &self.fd3_payload
    }

    #[must_use]
    pub(crate) fn argv(&self) -> Vec<OsString> {
        parse_fd3_payload_as_argv(&self.fd3_payload)
    }
}

pub(crate) const fn validate_timeout(timeout_ms: u64) -> Result<(), ConfigError> {
    if timeout_ms == 0 {
        return Err(ConfigError::TimeoutMustBePositive { timeout_ms });
    }
    Ok(())
}

pub(crate) fn validate_program_path(path: &Path) -> Result<(), ConfigError> {
    if !path.exists() {
        return Err(ConfigError::ProgramMissing { path: path.to_path_buf() });
    }

    let metadata = path
        .metadata()
        .map_err(|_| ConfigError::ProgramMissing { path: path.to_path_buf() })?;

    if !metadata.is_file() {
        return Err(ConfigError::ProgramMissing { path: path.to_path_buf() });
    }

    let permissions = metadata.permissions();
    if permissions.mode() & 0o111 == 0 {
        return Err(ConfigError::ProgramNotExecutable { path: path.to_path_buf() });
    }

    Ok(())
}

#[must_use]
pub(crate) fn parse_fd3_payload_as_argv(payload: &[u8]) -> Vec<OsString> {
    String::from_utf8_lossy(payload)
        .split_whitespace()
        .map(OsString::from)
        .collect()
}
