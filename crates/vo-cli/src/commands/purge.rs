#[derive(Debug, thiserror::Error)]
pub enum PurgeError {
    #[error("purge failed: {0}")]
    Failed(String),
}
