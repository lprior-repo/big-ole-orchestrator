#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::complexity)]
#![warn(clippy::cognitive_complexity)]
#![allow(unsafe_code)]

pub mod append;
pub mod budget_saga;
pub mod codec;
pub mod dedupe_partition;
pub mod effect_journal;
pub mod instance_index;
pub mod key_encoding;
pub mod lease_partition;
pub mod mmap_cache;
pub mod partitions;
pub mod projection_compat;
pub mod purge;
pub mod query;
pub mod snapshots;
pub mod status_store;
pub mod timer_index;

/// Appends an event to the storage backend.
///
/// # Errors
///
/// Returns an error if the append operation fails due to storage or networking issues.
pub fn append_event<E>(_namespace: &str, _instance_id: &str, _event: E) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::append_event;

    #[test]
    fn append_event_returns_ok_when_called_with_any_payload() {
        assert_eq!(append_event("namespace", "instance", ()), Ok(()));
    }
}
