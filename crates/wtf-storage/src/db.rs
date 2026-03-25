//! wtf-engine durable storage database.
//!
//! Provides a wrapper around `sled` with logic for multiple trees.

use std::path::Path;
use thiserror::Error;
use wtf_common::WtfError;

/// Storage error taxonomy for the durable engine.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Access denied to storage path: {0}")]
    AccessDenied(String),

    #[error("Database corrupted: {0}")]
    Corrupted(String),

    #[error("Database lock held by another process: {0}")]
    LockHeld(String),

    #[error("Failed to initialize storage: {0}")]
    InitializationFailed(String),

    #[error("IO failure: {0}")]
    Io(String),
}

impl From<StorageError> for WtfError {
    fn from(err: StorageError) -> Self {
        WtfError::sled_error(err.to_string())
    }
}

impl From<sled::Error> for StorageError {
    fn from(err: sled::Error) -> Self {
        match err {
            sled::Error::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                StorageError::AccessDenied(e.to_string())
            }
            sled::Error::Io(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                StorageError::LockHeld(e.to_string())
            }
            sled::Error::Io(e) => StorageError::Io(e.to_string()),
            sled::Error::Corruption { .. } => StorageError::Corrupted(err.to_string()),
            _ => StorageError::InitializationFailed(err.to_string()),
        }
    }
}

/// Logical tree names for the durable engine.
pub const INSTANCES: &[u8] = b"instances";
pub const JOURNAL: &[u8] = b"journal";
pub const TIMERS: &[u8] = b"timers";
pub const SIGNALS: &[u8] = b"signals";
pub const RUN_QUEUE: &[u8] = b"run_queue";
pub const ACTIVITIES: &[u8] = b"activities";
pub const WORKFLOWS: &[u8] = b"workflows";

/// Durable database holding all required tree handles.
pub struct Database {
    pub db: sled::Db,
    pub instances: sled::Tree,
    pub journal: sled::Tree,
    pub timers: sled::Tree,
    pub signals: sled::Tree,
    pub run_queue: sled::Tree,
    pub activities: sled::Tree,
    pub workflows: sled::Tree,
}

impl Database {
    /// Open a database at the given path, initializing all required trees.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the database cannot be opened or trees cannot be initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let db = sled::open(path)?;

        Ok(Self {
            instances: db.open_tree(INSTANCES)?,
            journal: db.open_tree(JOURNAL)?,
            timers: db.open_tree(TIMERS)?,
            signals: db.open_tree(SIGNALS)?,
            run_queue: db.open_tree(RUN_QUEUE)?,
            activities: db.open_tree(ACTIVITIES)?,
            workflows: db.open_tree(WORKFLOWS)?,
            db,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_database_persists_across_restart() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let path = dir.path();

        {
            let db = Database::open(path)?;
            db.instances.insert(b"k1", b"v1")?;
            db.db.flush()?;
        }

        {
            let db = Database::open(path)?;
            let val = db.instances.get(b"k1")?;
            assert_eq!(val.as_deref(), Some(&b"v1"[..]));
        }

        Ok(())
    }

    #[test]
    fn test_tree_isolation() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let db = Database::open(dir.path())?;

        db.instances.insert(b"k1", b"v1")?;

        assert!(db.journal.get(b"k1")?.is_none());
        assert!(db.timers.get(b"k1")?.is_none());
        assert!(db.signals.get(b"k1")?.is_none());
        assert!(db.run_queue.get(b"k1")?.is_none());
        assert!(db.activities.get(b"k1")?.is_none());
        assert!(db.workflows.get(b"k1")?.is_none());

        Ok(())
    }

    #[test]
    fn test_multi_tree_accessibility() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let db = Database::open(dir.path())?;

        db.instances.insert(b"i", b"1")?;
        db.journal.insert(b"j", b"2")?;
        db.timers.insert(b"t", b"3")?;
        db.signals.insert(b"s", b"4")?;
        db.run_queue.insert(b"r", b"5")?;
        db.activities.insert(b"a", b"6")?;
        db.workflows.insert(b"w", b"7")?;

        assert_eq!(db.instances.get(b"i")?.as_deref(), Some(&b"1"[..]));
        assert_eq!(db.journal.get(b"j")?.as_deref(), Some(&b"2"[..]));
        assert_eq!(db.timers.get(b"t")?.as_deref(), Some(&b"3"[..]));
        assert_eq!(db.signals.get(b"s")?.as_deref(), Some(&b"4"[..]));
        assert_eq!(db.run_queue.get(b"r")?.as_deref(), Some(&b"5"[..]));
        assert_eq!(db.activities.get(b"a")?.as_deref(), Some(&b"6"[..]));
        assert_eq!(db.workflows.get(b"w")?.as_deref(), Some(&b"7"[..]));

        Ok(())
    }

    #[test]
    fn test_error_path_is_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let file_path = dir.path().join("file");
        std::fs::write(&file_path, "not a directory")?;

        let res = Database::open(&file_path);
        assert!(matches!(
            res,
            Err(StorageError::InitializationFailed(_) | StorageError::Io(_))
        ));

        Ok(())
    }
}
