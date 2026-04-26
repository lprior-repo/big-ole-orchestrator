use crate::error::ConfigError;
use std::ffi::{CString, OsString};
use std::path::{Path, PathBuf};
use libc::{fstat, open, close, O_NOFOLLOW, O_RDONLY, S_IFMT, S_IFREG};

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
        let canonical_path = open_and_validate_program(p)?;

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

fn open_and_validate_program(path: &Path) -> Result<PathBuf, ConfigError> {
    let path_str = path.to_str().ok_or_else(|| ConfigError::ProgramMissing {
        path: path.to_path_buf(),
    })?;

    let c_path = CString::new(path_str).map_err(|_| ConfigError::ProgramMissing {
        path: path.to_path_buf(),
    })?;

    let fd = unsafe { open(c_path.as_ptr(), O_NOFOLLOW | O_RDONLY) };
    if fd < 0 {
        return Err(ConfigError::ProgramMissing {
            path: path.to_path_buf(),
        });
    }

    let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
    let fstat_result = unsafe { fstat(fd, &mut stat_buf) };
    let close_result = unsafe { close(fd) };

    if fstat_result < 0 {
        return Err(ConfigError::ProgramMissing {
            path: path.to_path_buf(),
        });
    }

    if close_result < 0 {
        return Err(ConfigError::ProgramMissing {
            path: path.to_path_buf(),
        });
    }

    if (stat_buf.st_mode & S_IFMT) != S_IFREG {
        return Err(ConfigError::ProgramMissing {
            path: path.to_path_buf(),
        });
    }

    if stat_buf.st_mode & 0o111 == 0 {
        return Err(ConfigError::ProgramNotExecutable {
            path: path.to_path_buf(),
        });
    }

    let canonical_path = path.canonicalize().map_err(|_| ConfigError::ProgramMissing {
        path: path.to_path_buf(),
    })?;

    Ok(canonical_path)
}

#[must_use]
pub(crate) fn parse_fd3_payload_as_argv(payload: &[u8]) -> Vec<OsString> {
    String::from_utf8_lossy(payload)
        .split_whitespace()
        .map(OsString::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn validate_timeout_returns_error_when_timeout_is_zero() {
        assert_eq!(
            validate_timeout(0),
            Err(ConfigError::TimeoutMustBePositive { timeout_ms: 0 })
        );
        assert_eq!(validate_timeout(10), Ok(()));
    }

    #[test]
    fn validate_program_returns_missing_when_path_does_not_exist() {
        let path = PathBuf::from("/does/not/exist");
        let result = open_and_validate_program(&path);
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ProgramMissing { path: p } => assert_eq!(p, path),
            other => panic!("expected ProgramMissing, got {:?}", other),
        }
    }

    #[test]
    fn validate_program_returns_missing_when_path_is_directory() {
        let dir = tempdir().unwrap();
        let result = open_and_validate_program(dir.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ProgramMissing { path: p } => assert_eq!(p, dir.path()),
            other => panic!("expected ProgramMissing, got {:?}", other),
        }
    }

    #[test]
    fn validate_program_rejects_non_executable_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("not_exec");
        File::create(&file_path).unwrap();
        let metadata = std::fs::metadata(&file_path).unwrap();
        let mut perms = metadata.permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();
        let result = open_and_validate_program(&file_path);
        assert!(result.is_err(), "non-executable file should be rejected");
    }

    #[test]
    fn validate_program_accepts_executable_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let metadata = std::fs::metadata(&file_path).unwrap();
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();
        let result = open_and_validate_program(&file_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), file_path.canonicalize().unwrap());
    }

    #[test]
    fn subprocess_config_returns_expected_getters_when_input_is_valid() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let config = SubprocessConfig::new(&file_path, 100, b"arg1 arg2".to_vec()).unwrap();
        assert_eq!(config.timeout_ms(), 100);
        assert_eq!(config.fd3_payload(), b"arg1 arg2");
        let argv = config.argv();
        assert_eq!(argv.len(), 2);
        assert_eq!(argv[0], "arg1");
        assert_eq!(argv[1], "arg2");
    }

    #[test]
    fn subprocess_config_supports_clone_eq_and_debug() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let config1 = SubprocessConfig::new(&file_path, 100, b"arg1 arg2".to_vec()).unwrap();
        let config2 = config1.clone();

        assert_eq!(config1, config2);

        let debug_str = format!("{:?}", config1);
        assert!(debug_str.contains("SubprocessConfig"));
    }
}
