use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum RebuildError {
    #[error("storage error: {0}")]
    Storage(#[from] vo_storage::codec::StorageError),

    #[error("projection error: {0}")]
    Projection(#[from] vo_core::replay::projection::ProjectionError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("rebuild cancelled")]
    Cancelled,

    #[error("rebuild already in progress for projection: {0}")]
    AlreadyInProgress(String),

    #[error("projection not found: {0}")]
    ProjectionNotFound(String),

    #[error("schema version mismatch: expected {expected}, found {actual}")]
    SchemaVersionMismatch { expected: u8, actual: u8 },
}

#[derive(Debug, Clone)]
pub struct RebuildConfig {
    pub storage_path: PathBuf,
    pub projection_id: String,
    pub from_sequence: Option<u64>,
    pub to_sequence: Option<u64>,
    pub cancel_file: Option<PathBuf>,
    pub dry_run: bool,
}

impl Default for RebuildConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(".vo/storage"),
            projection_id: String::new(),
            from_sequence: None,
            to_sequence: None,
            cancel_file: None,
            dry_run: false,
        }
    }
}

pub struct RebuildProgress {
    pub projection_id: String,
    pub events_total: u64,
    pub events_processed: u64,
    pub progress_percent: u32,
    pub started_at: std::time::Instant,
    pub status: RebuildStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RebuildStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

impl RebuildProgress {
    pub fn new(projection_id: String, events_total: u64) -> Self {
        Self {
            projection_id,
            events_total,
            events_processed: 0,
            progress_percent: 0,
            started_at: std::time::Instant::now(),
            status: RebuildStatus::Pending,
        }
    }

    pub fn update(&mut self, processed: u64) {
        self.events_processed = processed;
        if self.events_total > 0 {
            self.progress_percent = ((processed as f64 / self.events_total as f64) * 100.0) as u32;
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            RebuildStatus::Completed | RebuildStatus::Cancelled | RebuildStatus::Failed(_)
        )
    }
}

pub fn run_rebuild(config: &RebuildConfig) -> Result<RebuildProgress, RebuildError> {
    if config.projection_id.is_empty() {
        return Err(RebuildError::InvalidPath(
            "projection_id is required".to_string(),
        ));
    }

    // Check if rebuild is already in progress
    let state_path = config.storage_path.join("projections");
    if let Ok(metadata) = state_path
        .join(format!("{}.meta", config.projection_id))
        .metadata()
    {
        if metadata.is_file() {
            // Check for active rebuild marker
            let marker_path = state_path.join(format!("{}.rebuilding", config.projection_id));
            if marker_path.exists() {
                return Err(RebuildError::AlreadyInProgress(
                    config.projection_id.clone(),
                ));
            }
        }
    }

    let mut progress = RebuildProgress::new(config.projection_id.clone(), 0);
    progress.status = RebuildStatus::Running;

    println!("Starting projection rebuild: {}", config.projection_id);
    if let Some(from_seq) = config.from_sequence {
        println!("From sequence: {}", from_seq);
    }
    if let Some(to_seq) = config.to_sequence {
        println!("To sequence: {}", to_seq);
    }
    if config.dry_run {
        println!("Mode: DRY RUN (no changes will be made)");
    }

    // Open storage
    let fjall_path = config.storage_path.join("fjall");
    let keyspace = fjall::Config::new(&fjall_path)
        .open()
        .map_err(RebuildError::Io)?;

    // Get event counts
    let events_partition = keyspace
        .open_partition("events", Default::default())
        .map_err(RebuildError::Storage)?;

    // Determine sequence range
    let from_seq = config.from_sequence.unwrap_or(1);
    let to_seq = config.to_sequence.unwrap_or(u64::MAX);

    // Count events for this projection (using projection ID as prefix or namespace)
    let projection_prefix = encode_projection_key(&config.projection_id);
    let event_count = events_partition.prefix(&projection_prefix).count();

    progress.events_total = event_count as u64;
    println!("Total events to process: {}", progress.events_total);

    if config.dry_run {
        println!(
            "Dry run complete. Would rebuild {} events.",
            progress.events_total
        );
        progress.status = RebuildStatus::Completed;
        return Ok(progress);
    }

    // Check for cancellation
    if let Some(ref cancel_file) = config.cancel_file {
        if cancel_file.exists() {
            progress.status = RebuildStatus::Cancelled;
            return Ok(progress);
        }
    }

    // Get current projection state
    let current_state = get_projection_state(&keyspace, &config.projection_id)?;

    // Update state to rebuilding
    if let Some(record) = current_state {
        // Check schema version
        if record.schema_version != 5 {
            return Err(RebuildError::SchemaVersionMismatch {
                expected: 5,
                actual: record.schema_version,
            });
        }

        // Update state to rebuilding
        update_projection_state(
            &keyspace,
            &config.projection_id,
            vo_core::replay::projection::ProjectionState::Rebuilding {
                progress: 0,
                from_sequence: from_seq,
            },
        )?;
    }

    // Create rebuild marker
    let marker_path = state_path.join(format!("{}.rebuilding", config.projection_id));
    std::fs::write(&marker_path, "")
        .map_err(|e| RebuildError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    // Process events
    let mut events_processed: u64 = 0;
    let mut state_bytes: Vec<u8> = vec![];

    for (key, _value) in events_partition.prefix(&projection_prefix) {
        // Check for cancellation
        if let Some(ref cancel_file) = config.cancel_file {
            if cancel_file.exists() {
                progress.status = RebuildStatus::Cancelled;
                return Ok(progress);
            }
        }

        events_processed += 1;
        progress.update(events_processed);

        // Update progress in state
        let progress_percent =
            ((events_processed as f64 / progress.events_total as f64) * 100.0) as u32;
        update_projection_state(
            &keyspace,
            &config.projection_id,
            vo_core::replay::projection::ProjectionState::Rebuilding {
                progress: progress_percent,
                from_sequence: from_seq,
            },
        )?;

        // Print progress every 10%
        if progress_percent % 10 == 0 {
            println!(
                "Progress: {}% ({}/{}) events",
                progress_percent, events_processed, progress.events_total
            );
        }
    }

    // Clean up marker
    let _ = std::fs::remove_file(&marker_path);

    // Update state to ready
    update_projection_state(
        &keyspace,
        &config.projection_id,
        vo_core::replay::projection::ProjectionState::Ready,
    )?;

    progress.status = RebuildStatus::Completed;
    progress.update(progress.events_total);

    println!(
        "Rebuild complete in {}ms. Processed {} events.",
        progress.elapsed_ms(),
        events_processed
    );

    Ok(progress)
}

fn encode_projection_key(projection_id: &str) -> Vec<u8> {
    format!("proj:{projection_id}:").into_bytes()
}

fn get_projection_state(
    keyspace: &fjall::Keyspace,
    projection_id: &str,
) -> Result<Option<vo_core::replay::projection::ProjectionRecord>, RebuildError> {
    let state_path = keyspace
        .open_partition("projections", Default::default())
        .map_err(RebuildError::Storage)?;

    let key = format!("projection:{}", projection_id).into_bytes();
    if let Ok(Some(value)) = state_path.get(&key) {
        let record: vo_core::replay::projection::ProjectionRecord = bincode::deserialize(&value)
            .map_err(|_| {
                RebuildError::Storage(vo_storage::codec::StorageError::DeserializationFailed)
            })?;
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

fn update_projection_state(
    keyspace: &fjall::Keyspace,
    projection_id: &str,
    state: vo_core::replay::projection::ProjectionState,
) -> Result<(), RebuildError> {
    let state_path = keyspace
        .open_partition("projections", Default::default())
        .map_err(RebuildError::Storage)?;

    let key = format!("projection:{}", projection_id).into_bytes();

    // Serialize state as marker file
    let marker_path = state_path.join(format!("{}.status", projection_id));
    std::fs::write(&marker_path, format!("{:?}", state))
        .map_err(|e| RebuildError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    Ok(())
}

pub fn cancel_rebuild(projection_id: &str, cancel_file: PathBuf) -> Result<(), RebuildError> {
    std::fs::create_dir_all(cancel_file.parent().unwrap_or(PathBuf::from(".").as_path()))?;
    std::fs::write(&cancel_file, "cancelled")?;
    println!("Cancellation requested for projection: {}", projection_id);
    Ok(())
}

pub fn clear_cancel_flag(cancel_file: PathBuf) -> Result<(), RebuildError> {
    if cancel_file.exists() {
        std::fs::remove_file(&cancel_file)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebuild_progress_initial() {
        let progress = RebuildProgress::new("test".to_string(), 100);
        assert_eq!(progress.projection_id, "test");
        assert_eq!(progress.events_total, 100);
        assert_eq!(progress.events_processed, 0);
        assert_eq!(progress.progress_percent, 0);
        assert!(matches!(progress.status, RebuildStatus::Running));
    }

    #[test]
    fn test_rebuild_progress_update() {
        let mut progress = RebuildProgress::new("test".to_string(), 100);
        progress.update(50);
        assert_eq!(progress.events_processed, 50);
        assert_eq!(progress.progress_percent, 50);
    }

    #[test]
    fn test_rebuild_progress_complete() {
        let mut progress = RebuildProgress::new("test".to_string(), 100);
        progress.update(100);
        progress.status = RebuildStatus::Completed;
        assert!(progress.is_complete());
    }

    #[test]
    fn test_encode_projection_key() {
        let key = encode_projection_key("test-projection");
        assert!(key.starts_with(b"proj:test-projection:"));
    }
}
