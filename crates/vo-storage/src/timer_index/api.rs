//! Stub module for timer API functions.
//! Placeholder for future timer API implementation.

use crate::codec::StorageError;
use crate::timer_index::ScanResult;

/// Stub: return an empty timer set.
pub fn timer_set(_db: &fjall::Database) -> Result<ScanResult, StorageError> {
    Ok(vec![])
}

/// Stub: no-op delete.
pub fn timer_delete(_db: &fjall::Database, _key: &[u8]) -> Result<(), StorageError> {
    Ok(())
}

/// Stub: return empty scan result.
pub fn scan_due_timers(_db: &fjall::Database, _now_ms: u64) -> Result<ScanResult, StorageError> {
    Ok(vec![])
}

/// Stub: return empty poll result.
pub fn poll_expired_timers(
    _db: &fjall::Database,
    _now_ms: u64,
    _max_count: usize,
) -> Result<ScanResult, StorageError> {
    Ok(vec![])
}

/// Stub: return empty scan result.
pub fn scan_all_timers_for_instance(
    _db: &fjall::Database,
    _instance_id: &[u8],
) -> Result<ScanResult, StorageError> {
    Ok(vec![])
}
