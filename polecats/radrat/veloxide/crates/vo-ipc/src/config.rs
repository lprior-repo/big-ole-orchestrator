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

        let canonical_path = p.canonicalize().map_err(|_| ConfigError::ProgramMissing {
            path: p.to_path_buf(),
        })?;

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
        return Err(ConfigError::ProgramMissing {
            path: path.to_path_buf(),
        });
    }

    let metadata = path.metadata().map_err(|_| ConfigError::ProgramMissing {
        path: path.to_path_buf(),
    })?;

    if !metadata.is_file() {
        return Err(ConfigError::ProgramMissing {
            path: path.to_path_buf(),
        });
    }

    let permissions = metadata.permissions();
    if permissions.mode() & 0o111 == 0 {
        return Err(ConfigError::ProgramNotExecutable {
            path: path.to_path_buf(),
        });
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
    fn validate_program_path_returns_missing_when_path_does_not_exist() {
        let path = PathBuf::from("/does/not/exist");
        assert_eq!(
            validate_program_path(&path),
            Err(ConfigError::ProgramMissing { path })
        );
    }

    #[test]
    fn validate_program_path_returns_missing_when_path_is_directory() {
        let dir = tempdir().unwrap();
        assert_eq!(
            validate_program_path(dir.path()),
            Err(ConfigError::ProgramMissing {
                path: dir.path().to_path_buf(),
            })
        );
    }

    #[test]
    fn validate_program_path_returns_not_executable_when_permission_bits_missing() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("not_exec");
        File::create(&file_path).unwrap();
        let metadata = std::fs::metadata(&file_path).unwrap();
        let mut perms = metadata.permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();
        assert_eq!(
            validate_program_path(&file_path),
            Err(ConfigError::ProgramNotExecutable { path: file_path })
        );
    }

    #[test]
    fn validate_program_path_accepts_executable_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let metadata = std::fs::metadata(&file_path).unwrap();
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();
        assert_eq!(validate_program_path(&file_path), Ok(()));
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
