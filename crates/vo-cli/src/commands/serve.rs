use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("invalid host: {0}")]
    InvalidHost(String),
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("invalid storage path: {0}")]
    InvalidStoragePath(String),
}

pub struct ServeConfig {
    pub host: String,
    pub port: u16,
    pub storage_path: PathBuf,
}

pub fn validate_serve_config(config: &ServeConfig) -> Result<(), ServeError> {
    if config.host.is_empty() {
        return Err(ServeError::InvalidHost(
            "host must not be empty".to_string(),
        ));
    }
    if config.port == 0 {
        return Err(ServeError::InvalidPort(
            "port must be greater than 0".to_string(),
        ));
    }
    if !config.storage_path.exists() {
        return Err(ServeError::InvalidStoragePath(format!(
            "storage path does not exist: {}",
            config.storage_path.display()
        )));
    }
    if !config.storage_path.is_dir() {
        return Err(ServeError::InvalidStoragePath(format!(
            "storage path is not a directory: {}",
            config.storage_path.display()
        )));
    }
    Ok(())
}

pub async fn run_serve(config: &ServeConfig) -> Result<(), ServeError> {
    validate_serve_config(config)?;
    println!(
        "Starting veloxide server on {}:{} with storage at {}",
        config.host,
        config.port,
        config.storage_path.display()
    );
    Ok(())
}
