pub mod bloom;
pub mod query;
pub mod storage;
#[cfg(test)]
mod tests;
pub mod types;

use std::sync::Arc;

use crate::partitions::{get_partition_config, TIMERS_PARTITION};

pub use bloom::{BloomStats, TimerBloomFilter};
pub use query::{scan_all_timers_for_instance, scan_due_timers};
pub use storage::{poll_expired_timers, timer_delete, timer_set, ScanResult, Storage};
pub use types::{TimerKey, TimerRecord, TimerValue};

pub struct FjallTimerIndex {
    db: Arc<fjall::Database>,
    partition: Arc<fjall::Keyspace>,
}

impl FjallTimerIndex {
    pub fn open(db: &fjall::Database) -> Result<Self, crate::codec::StorageError> {
        let config = get_partition_config(TIMERS_PARTITION);
        let partition = db
            .keyspace(TIMERS_PARTITION, || config.to_fjall_options())
            .map_err(|_| crate::codec::StorageError::Storage)?;
        Ok(Self {
            db: Arc::new(db.clone()),
            partition: Arc::new(partition),
        })
    }

    #[must_use]
    pub fn keyspace(&self) -> &fjall::Keyspace {
        &self.partition
    }
}
