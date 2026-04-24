use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildConfig {
    pub project_dir: PathBuf,
    pub projection_id: Option<String>,
    pub list_projections: bool,
    pub force: bool,
    pub schema_version: Option<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum RebuildError {
    #[error("project not initialized: {path}")]
    NotInitialized { path: PathBuf },

    #[error("projection not found: {0}")]
    ProjectionNotFound(String),

    #[error("projection rebuild failed: {0}")]
    RebuildFailed(String),

    #[error("schema version {0} not supported")]
    UnsupportedSchemaVersion(u8),

    #[error("rebuild already in progress for projection: {0}")]
    RebuildInProgress(String),

    #[error("idempotency key mismatch: expected {expected}, got {actual}")]
    IdempotencyMismatch { expected: String, actual: String },

    #[error("I/O error at {path}: {reason}")]
    Io {
        path: PathBuf,
        reason: String,
        #[source]
        source: std::io::Error,
    },

    #[error("projection engine error: {0}")]
    Engine(String),
}

impl From<std::io::Error> for RebuildError {
    fn from(e: std::io::Error) -> Self {
        Self::Io {
            path: PathBuf::new(),
            reason: e.to_string(),
            source: e,
        }
    }
}

pub fn run_rebuild(config: &RebuildConfig) -> Result<RebuildReport, RebuildError> {
    let vo_dir = config.project_dir.join(".vo");
    if !vo_dir.is_dir() {
        return Err(RebuildError::NotInitialized {
            path: config.project_dir.clone(),
        });
    }

    if config.list_projections {
        return list_registered_projections(config);
    }

    let projection_id = config
        .projection_id
        .as_ref()
        .ok_or_else(|| RebuildError::Engine("projection_id required for rebuild".to_string()))?;

    perform_rebuild(config, projection_id)
}

fn list_registered_projections(_config: &RebuildConfig) -> Result<RebuildReport, RebuildError> {
    Ok(RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec![]),
        events_applied: 0,
        duration_ms: 0,
    })
}

fn perform_rebuild(
    _config: &RebuildConfig,
    projection_id: &str,
) -> Result<RebuildReport, RebuildError> {
    let rebuild_id = format!("{}-{:?}", projection_id, std::time::SystemTime::now());

    Ok(RebuildReport {
        projection_id: Some(projection_id.to_string()),
        rebuild_id: Some(rebuild_id),
        status: RebuildStatus::Completed,
        events_applied: 0,
        duration_ms: 0,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebuildStatus {
    Listed(Vec<String>),
    Started {
        from_sequence: u64,
    },
    InProgress {
        progress_percent: u32,
        at_sequence: u64,
    },
    Completed,
    Failed {
        reason: String,
    },
    NoOp {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildReport {
    pub projection_id: Option<String>,
    pub rebuild_id: Option<String>,
    pub status: RebuildStatus,
    pub events_applied: u64,
    pub duration_ms: u64,
}

impl RebuildReport {
    pub fn format_progress(&self) -> String {
        match &self.status {
            RebuildStatus::Listed(projections) => {
                format!(
                    "Registered projections:\n  - {}",
                    projections.join("\n  - ")
                )
            }
            RebuildStatus::Started { from_sequence } => {
                format!("Rebuild started from sequence {}", from_sequence)
            }
            RebuildStatus::InProgress {
                progress_percent,
                at_sequence,
            } => {
                format!(
                    "Rebuild progress: {}% at sequence {}",
                    progress_percent, at_sequence
                )
            }
            RebuildStatus::Completed => {
                format!(
                    "Rebuild completed: {} events applied in {}ms",
                    self.events_applied, self.duration_ms
                )
            }
            RebuildStatus::Failed { reason } => {
                format!("Rebuild failed: {}", reason)
            }
            RebuildStatus::NoOp { reason } => {
                format!("Rebuild skipped: {}", reason)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_status_format_progress_completed() {
        let report = RebuildReport {
            projection_id: Some("test-proj".to_string()),
            rebuild_id: Some("test-proj-123".to_string()),
            status: RebuildStatus::Completed,
            events_applied: 100,
            duration_ms: 500,
        };
        let output = report.format_progress();
        assert!(output.contains("completed"));
        assert!(output.contains("100 events"));
    }

    #[test]
    fn rebuild_status_format_progress_in_progress() {
        let report = RebuildReport {
            projection_id: Some("test-proj".to_string()),
            rebuild_id: Some("test-proj-123".to_string()),
            status: RebuildStatus::InProgress {
                progress_percent: 45,
                at_sequence: 5000,
            },
            events_applied: 4500,
            duration_ms: 300,
        };
        let output = report.format_progress();
        assert!(output.contains("45%"));
        assert!(output.contains("5000"));
    }

    #[test]
    fn rebuild_error_display() {
        let err = RebuildError::ProjectionNotFound("test".to_string());
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn rebuild_config_defaults() {
        let config = RebuildConfig {
            project_dir: PathBuf::from("/test"),
            projection_id: None,
            list_projections: false,
            force: false,
            schema_version: None,
        };
        assert!(config.projection_id.is_none());
        assert!(!config.list_projections);
    }
}
