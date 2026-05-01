use crate::error::ConfigError;
use std::ffi::{CString, OsString};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use libc::{close, fstat, open, O_NOFOLLOW, O_RDONLY, S_IFMT, S_IFREG};

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

fn validate_program_path(path: &Path) -> Result<(), ConfigError> {
    let path_cstr = CString::new(path.to_str().ok_or_else(|| ConfigError::ProgramMissing {
        path: path.to_path_buf(),
    })?).map_err(|_| ConfigError::ProgramMissing {
        path: path.to_path_buf(),
    })?;

    let fd = unsafe { open(path_cstr.as_ptr(), O_NOFOLLOW | O_RDONLY) };
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
    use tempfile::tempdir;

    #[test]
    fn validate_timeout_rejects_zero() {
        assert_eq!(
            validate_timeout(0),
            Err(ConfigError::TimeoutMustBePositive { timeout_ms: 0 })
        );
    }

    #[test]
    fn validate_timeout_accepts_positive() {
        assert_eq!(validate_timeout(1), Ok(()));
        assert_eq!(validate_timeout(u64::MAX), Ok(()));
    }

    #[test]
    fn parse_fd3_payload_splits_whitespace() {
        let args = parse_fd3_payload_as_argv(b"hello world  foo");
        assert_eq!(args, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn parse_fd3_payload_empty_returns_empty() {
        let args = parse_fd3_payload_as_argv(b"");
        assert!(args.is_empty());
    }

    #[test]
    fn parse_fd3_payload_all_whitespace_returns_empty() {
        let args = parse_fd3_payload_as_argv(b"  \t  \n  ");
        assert!(args.is_empty());
    }

    #[test]
    fn parse_fd3_payload_invalid_utf8_lossy() {
        let args = parse_fd3_payload_as_argv(&[0xff, 0xfe, b' ', b'a']);
        assert_eq!(args.len(), 2);
        assert_eq!(args[1], "a");
    }

    #[test]
    fn subprocess_config_validates_timeout() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let result = SubprocessConfig::new(&file_path, 0, vec![]);
        assert!(matches!(result, Err(ConfigError::TimeoutMustBePositive { .. })));
    }

    #[test]
    fn subprocess_config_validates_missing_path() {
        let result = SubprocessConfig::new("/nonexistent/binary", 100, vec![]);
        assert!(matches!(result, Err(ConfigError::ProgramMissing { .. })));
    }

    #[test]
    fn subprocess_config_validates_non_executable() {
        let dir = std::env::temp_dir().join("vo_ipc_test_nonexec");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("data.txt");
        let _ = std::fs::remove_file(&file_path);
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let result = SubprocessConfig::new(&file_path, 100, vec![]);
        assert!(matches!(result, Err(ConfigError::ProgramNotExecutable { .. })));
        let _ = std::fs::remove_file(&file_path);
    }

    #[test]
    fn subprocess_config_validates_directory_as_not_a_file() {
        let dir = tempdir().unwrap();
        let result = SubprocessConfig::new(dir.path(), 100, vec![]);
        assert!(matches!(result, Err(ConfigError::ProgramMissing { .. })));
    }

    #[test]
    fn subprocess_config_getters_return_correct_values() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let config = SubprocessConfig::new(&file_path, 200, b"arg1 arg2".to_vec()).unwrap();
        assert_eq!(config.timeout_ms(), 200);
        assert_eq!(config.fd3_payload(), b"arg1 arg2");
        assert_eq!(config.argv(), vec!["arg1", "arg2"]);
    }

    #[test]
    fn subprocess_config_canonicalizes_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let config = SubprocessConfig::new(&file_path, 100, vec![]).unwrap();
        assert!(config.executable_path().is_absolute());
    }

    #[test]
    fn subprocess_config_clone_eq() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let config = SubprocessConfig::new(&file_path, 100, b"test".to_vec()).unwrap();
        let clone = config.clone();
        assert_eq!(config, clone);
    }

    #[test]
    fn subprocess_config_debug() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let config = SubprocessConfig::new(&file_path, 100, vec![]).unwrap();
        let debug = format!("{:?}", config);
        assert!(debug.contains("SubprocessConfig"));
    }

    #[test]
    fn subprocess_config_large_timeout_accepted() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let config = SubprocessConfig::new(&file_path, u64::MAX, vec![]).unwrap();
        assert_eq!(config.timeout_ms(), u64::MAX);
    }

    #[test]
    fn subprocess_config_payload_preserved() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exec");
        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let payload = b"custom payload with spaces".to_vec();
        let config = SubprocessConfig::new(&file_path, 500, payload.clone()).unwrap();
        assert_eq!(config.fd3_payload(), &payload[..]);
    }

    #[test]
    fn parse_fd3_payload_preserves_order() {
        let args = parse_fd3_payload_as_argv(b"first second third fourth");
        assert_eq!(args, vec!["first", "second", "third", "fourth"]);
    }

    #[test]
    fn parse_fd3_payload_single_arg() {
        let args = parse_fd3_payload_as_argv(b"only");
        assert_eq!(args, vec!["only"]);
    }

    #[test]
    fn parse_fd3_payload_tabs_and_newlines() {
        let args = parse_fd3_payload_as_argv(b"a\tb\nc\td");
        assert_eq!(args, vec!["a", "b", "c", "d"]);
    }
}
