//! Heartbeat expiry watcher (bead wtf-r4aa).
//!
//! Watches the `wtf-heartbeats` NATS KV bucket (ADR-014) for entry deletions
//! caused by TTL expiry. When an entry expires, the watcher sends
//! `OrchestratorMsg::HeartbeatExpired { instance_id }` to the `MasterOrchestrator`,
//! which decides whether to trigger crash recovery.
//!
//! # Architecture
//! - The `wtf-heartbeats` bucket has `max_age = 10s`. Each live `WorkflowInstance`
//!   calls `write_heartbeat()` every ≤5s to refresh its entry.
//! - When the entry expires (TTL elapsed), NATS emits a `Delete` operation on the
//!   bucket's watch stream.
//! - This watcher sees the deletion and notifies the orchestrator.
//!
//! # Key format
//! Keys: `hb/<instance_id>` (see `wtf_storage::kv::heartbeat_key`).
//! Parsing: split on '/' and take index 1 to get the instance ID.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use async_nats::jetstream::kv::{Operation, Store};
use futures::StreamExt;
use ractor::ActorRef;
use wtf_common::InstanceId;

use crate::messages::OrchestratorMsg;

/// Parse an `InstanceId` from a heartbeat KV key (`hb/<instance_id>`).
///
/// Returns `None` if the key format is unexpected (e.g. from a different bucket).
#[must_use]
pub fn instance_id_from_heartbeat_key(key: &str) -> Option<InstanceId> {
    key.strip_prefix("hb/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .map(|id| InstanceId::new(id.to_owned()))
}

/// Run the heartbeat expiry watcher loop until `shutdown_rx` fires.
///
/// Watches the `wtf-heartbeats` KV bucket for TTL-expired entries. For each
/// expired entry (operation = `Delete`), sends `HeartbeatExpired` to the
/// `orchestrator` actor.
///
/// Spawn this as a background Tokio task:
/// ```ignore
/// tokio::spawn(run_heartbeat_watcher(heartbeats, orchestrator_ref, shutdown_rx));
/// ```
///
/// # Errors
/// Returns an error string if the initial `watch_all()` call fails (unrecoverable).
/// Per-entry errors are logged and the loop continues.
pub async fn run_heartbeat_watcher(
    heartbeats: Store,
    orchestrator: ActorRef<OrchestratorMsg>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let mut watch = heartbeats
        .watch_all()
        .await
        .map_err(|e| format!("heartbeat watch_all failed: {e}"))?;

    tracing::info!("heartbeat expiry watcher started");

    loop {
        tokio::select! {
            entry = watch.next() => {
                let entry_processed = match entry {
                    Some(res) => process_heartbeat_entry(Some(res.map_err(|e| e.to_string())), &orchestrator),
                    None => process_heartbeat_entry(None::<Result<async_nats::jetstream::kv::Entry, String>>, &orchestrator),
                };
                if !entry_processed {
                    break;
                }
            }
            _ = shutdown_rx.changed() => {
                tracing::info!("heartbeat watcher shutting down");
                break;
            }
        }
    }

    Ok(())
}

fn process_heartbeat_entry<E: std::fmt::Display>(
    entry: Option<Result<async_nats::jetstream::kv::Entry, E>>,
    orchestrator: &ActorRef<OrchestratorMsg>,
) -> bool {
    match entry {
        None => {
            tracing::info!("heartbeat watch stream closed — stopping");
            false
        }
        Some(Err(e)) => {
            tracing::warn!(error = %e, "heartbeat watch error — continuing");
            true
        }
        Some(Ok(kv_entry)) => {
            if matches!(kv_entry.operation, Operation::Delete | Operation::Purge) {
                if let Some(instance_id) = instance_id_from_heartbeat_key(&kv_entry.key) {
                    tracing::debug!(instance_id = %instance_id, "heartbeat expired — notifying orchestrator");
                    let _ = orchestrator.cast(OrchestratorMsg::HeartbeatExpired { instance_id });
                } else {
                    tracing::warn!(key = %kv_entry.key, "unexpected heartbeat key format");
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_heartbeat_key() {
        let result = instance_id_from_heartbeat_key("hb/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        assert_eq!(
            result.map(|id| id.as_str().to_owned()),
            Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned())
        );
    }

    #[test]
    fn parse_short_instance_id() {
        let result = instance_id_from_heartbeat_key("hb/inst-001");
        assert_eq!(
            result.map(|id| id.as_str().to_owned()),
            Some("inst-001".to_owned())
        );
    }

    #[test]
    fn parse_key_missing_hb_prefix_returns_none() {
        let result = instance_id_from_heartbeat_key("instance/01ARZ");
        assert!(result.is_none());
    }

    #[test]
    fn parse_empty_key_returns_none() {
        let result = instance_id_from_heartbeat_key("");
        assert!(result.is_none());
    }

    #[test]
    fn parse_hb_prefix_only_returns_none() {
        let result = instance_id_from_heartbeat_key("hb/");
        assert!(result.is_none());
    }

    #[test]
    fn parse_hb_key_with_underscores() {
        let result = instance_id_from_heartbeat_key("hb/order_flow_01ARZ");
        assert_eq!(
            result.map(|id| id.as_str().to_owned()),
            Some("order_flow_01ARZ".to_owned())
        );
    }

    #[test]
    fn parse_hb_key_with_extra_segment_returns_none() {
        let result = instance_id_from_heartbeat_key("hb/inst-001/extra");
        assert!(result.is_none());
    }

    // run_heartbeat_watcher requires a live NATS server — covered by integration tests.
}
