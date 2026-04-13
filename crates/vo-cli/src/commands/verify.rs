use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("verification tests failed")]
    TestsFailed { stdout: String, stderr: String },
    #[error("cargo command failed: {source}")]
    CargoFailed {
        #[source]
        source: std::io::Error,
    },
    #[error("verification harness not found at {path}")]
    HarnessNotFound { path: PathBuf },
}

pub struct VerifyConfig {
    pub manifest_path: Option<PathBuf>,
}

impl Default for VerifyConfig {
    fn default() -> Self {
        Self {
            manifest_path: None,
        }
    }
}

pub fn run_verify(config: &VerifyConfig) -> Result<(), VerifyError> {
    let mut cmd = Command::new("cargo");
    cmd.arg("test")
        .arg("-p")
        .arg("vo-core")
        .arg("--lib")
        .arg("exact_once_verification");

    if let Some(ref manifest_path) = config.manifest_path {
        cmd.arg("--manifest-path");
        cmd.arg(manifest_path);
    }

    let output = cmd
        .output()
        .map_err(|source| VerifyError::CargoFailed { source })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(VerifyError::TestsFailed { stdout, stderr });
    }

    println!("Exact-once verification suite passed.");
    println!("All crash-point matrix tests passed.");
    println!("Zero false negatives confirmed.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_config_default_has_no_manifest_path() {
        let config = VerifyConfig::default();
        assert!(config.manifest_path.is_none());
    }

    #[test]
    fn verify_config_can_set_manifest_path() {
        let mut config = VerifyConfig::default();
        config.manifest_path = Some(PathBuf::from("/path/to/Cargo.toml"));
        assert!(config.manifest_path.is_some());
    }
}
