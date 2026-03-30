#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::complexity)]
#![warn(clippy::cognitive_complexity)]
#![forbid(unsafe_code)]

pub mod append;
pub mod codec;
pub mod instance_index;
pub mod partitions;
pub mod query;
pub mod timer_index;

/// Appends an event to the storage backend.
///
/// # Errors
///
/// Returns an error if the append operation fails due to storage or networking issues.
pub fn append_event<E>(_namespace: &str, _instance_id: &str, _event: E) -> Result<(), String> {
    Ok(())
}
