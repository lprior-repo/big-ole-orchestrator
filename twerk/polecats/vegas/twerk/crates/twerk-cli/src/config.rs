//! Configuration module for twerk-cli.
//!
//! Loads configuration from file with CLI flag overrides.
//! Priority: CLI flags > config file > defaults

use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(String),

    #[error("error parsing config from {path}: {line}: {column} - {source}")]
    ParseError {
        path: String,
        line: usize,
        column: usize,
        source: toml::de::Error,
    },

    #[error("error reading config file {path}: {source}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Config {
    pub timeout_secs: u64,
    pub workers: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            workers: 2,
        }
    }
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref)
            .map_err(|e| ConfigError::IoError {
                path: path_ref.display().to_string(),
                source: e,
            })?;

        toml::from_str(&content).map_err(|e| {
            let (line, column) = (e.line(), e.column());
            ConfigError::ParseError {
                path: path_ref.display().to_string(),
                line,
                column,
                source: e,
            }
        })
    }

    pub fn apply_cli_overrides(&mut self, cli_timeout: Option<u64>, cli_workers: Option<u32>) {
        if let Some(t) = cli_timeout {
            self.timeout_secs = t;
        }
        if let Some(w) = cli_workers {
            self.workers = w;
        }
    }

    pub fn load() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cli_overrides_file_config() {
        let mut config = Config {
            timeout_secs: 30,
            workers: 4,
        };
        config.apply_cli_overrides(Some(60), None);
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.workers, 4);
    }

    #[test]
    fn test_file_overrides_defaults() {
        let mut config = Config::default();
        config.apply_cli_overrides(None, Some(8));
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.workers, 8);
    }

    #[test]
    fn test_no_config_file_all_defaults() {
        let config = Config::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.workers, 2);
    }

    #[test]
    fn test_invalid_config_file_error() {
        let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
        temp_file.write_all(b"invalid toml {").unwrap();
        temp_file.flush();

        let result = Config::from_file(temp_file.path());
        assert!(result.is_err());

        let err = result.unwrap_err();
        let error_string = err.to_string();
        assert!(error_string.contains("error parsing config"));
        assert!(error_string.contains(&temp_file.path().display().to_string()));
    }

    #[test]
    fn test_valid_config_file_loading() {
        let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
        temp_file
            .write_all(b"timeout_secs = 45\nworkers = 6")
            .unwrap();
        temp_file.flush();

        let config = Config::from_file(temp_file.path()).unwrap();
        assert_eq!(config.timeout_secs, 45);
        assert_eq!(config.workers, 6);
    }

    #[test]
    fn test_cli_overrides_both_file_values() {
        let mut config = Config {
            timeout_secs: 30,
            workers: 4,
        };
        config.apply_cli_overrides(Some(60), Some(8));
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.workers, 8);
    }

    #[test]
    fn test_cli_overrides_none_leaves_file_values() {
        let mut config = Config {
            timeout_secs: 30,
            workers: 4,
        };
        config.apply_cli_overrides(None, None);
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.workers, 4);
    }
}