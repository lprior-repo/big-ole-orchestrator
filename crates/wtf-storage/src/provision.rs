//! `JetStream` stream provisioning — idempotent setup for all wtf-engine streams (ADR-013).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use async_nats::jetstream::{
    stream::{Config as StreamConfig, RetentionPolicy, StorageType},
    Context,
};
use wtf_common::WtfError;

/// Provision all four `JetStream` streams if they don't already exist.
///
/// This function is idempotent — safe to call on every engine startup.
/// If a stream already exists with a compatible config, `create_stream` returns it unchanged.
///
/// # Streams
/// - `wtf-events`: immutable event log, all workflow state transitions (source of truth)
/// - `wtf-work`: work queue for activity dispatch to workers
/// - `wtf-signals`: signal delivery to workflow instances
/// - `wtf-archive`: long-term retention of completed workflow logs
///
/// # Errors
/// Returns [`WtfError::NatsPublish`] if any stream creation fails.
pub async fn provision_streams(js: &Context) -> Result<(), WtfError> {
    create_events_stream(js).await?;
    create_work_stream(js).await?;
    create_signals_stream(js).await?;
    create_archive_stream(js).await?;
    Ok(())
}

/// wtf-events: the immutable event log — source of truth for all workflow state (ADR-013).
///
/// Subjects: `wtf.log.>` (namespace + `instance_id` encoded in subject segments).
/// Retention: Limits (keep all events up to `max_age`).
/// Max age: 90 days (configurable).
async fn create_events_stream(js: &Context) -> Result<(), WtfError> {
    js.create_stream(StreamConfig {
        name: "wtf-events".to_owned(),
        subjects: vec!["wtf.log.>".to_owned()],
        storage: StorageType::File,
        num_replicas: 1, // Override to 3 in production via EngineConfig
        retention: RetentionPolicy::Limits,
        max_age: Duration::from_secs(90 * 24 * 60 * 60), // 90 days
        max_message_size: 1024 * 1024,                   // 1 MiB max per event
        ..Default::default()
    })
    .await
    .map_err(|e| WtfError::nats_publish(format!("create wtf-events stream: {e}")))?;

    Ok(())
}

/// wtf-work: work queue for dispatching activities to workers.
///
/// Subjects: `wtf.work.>` (`activity_type` encoded in subject).
/// Retention: `WorkQueue` (each message delivered exactly once, then deleted).
async fn create_work_stream(js: &Context) -> Result<(), WtfError> {
    js.create_stream(StreamConfig {
        name: "wtf-work".to_owned(),
        subjects: vec!["wtf.work.>".to_owned()],
        storage: StorageType::File,
        num_replicas: 1,
        retention: RetentionPolicy::WorkQueue,
        ..Default::default()
    })
    .await
    .map_err(|e| WtfError::nats_publish(format!("create wtf-work stream: {e}")))?;

    Ok(())
}

/// wtf-signals: signal delivery to workflow instances.
///
/// Subjects: `wtf.signals.>`.
/// Retention: Interest (messages deleted after all consumers have received them).
async fn create_signals_stream(js: &Context) -> Result<(), WtfError> {
    js.create_stream(StreamConfig {
        name: "wtf-signals".to_owned(),
        subjects: vec!["wtf.signals.>".to_owned()],
        storage: StorageType::File,
        num_replicas: 1,
        retention: RetentionPolicy::Interest,
        ..Default::default()
    })
    .await
    .map_err(|e| WtfError::nats_publish(format!("create wtf-signals stream: {e}")))?;

    Ok(())
}

/// wtf-archive: long-term storage for completed workflow event logs.
///
/// Subjects: `wtf.archive.>`.
/// Max age: 365 days.
async fn create_archive_stream(js: &Context) -> Result<(), WtfError> {
    js.create_stream(StreamConfig {
        name: "wtf-archive".to_owned(),
        subjects: vec!["wtf.archive.>".to_owned()],
        storage: StorageType::File,
        num_replicas: 1,
        retention: RetentionPolicy::Limits,
        max_age: Duration::from_secs(365 * 24 * 60 * 60), // 365 days
        ..Default::default()
    })
    .await
    .map_err(|e| WtfError::nats_publish(format!("create wtf-archive stream: {e}")))?;

    Ok(())
}

/// Verify all expected streams exist. Returns `Ok(())` if all are present.
///
/// Used by health check endpoint to confirm storage is provisioned.
///
/// # Errors
/// Returns [`WtfError::NatsPublish`] if any stream is missing.
pub async fn verify_streams(js: &Context) -> Result<(), WtfError> {
    for name in ["wtf-events", "wtf-work", "wtf-signals", "wtf-archive"] {
        js.get_stream(name)
            .await
            .map_err(|e| WtfError::nats_publish(format!("stream {name} not found: {e}")))?;
    }
    Ok(())
}

/// Stream names used by wtf-engine.
pub mod stream_names {
    pub const EVENTS: &str = "wtf-events";
    pub const WORK: &str = "wtf-work";
    pub const SIGNALS: &str = "wtf-signals";
    pub const ARCHIVE: &str = "wtf-archive";
}

/// Subject prefixes for each stream.
pub mod subjects {
    /// Event log subject: `wtf.log.<namespace>.<instance_id>`
    pub const EVENTS_PREFIX: &str = "wtf.log";
    /// Work queue subject: `wtf.work.<activity_type>`
    pub const WORK_PREFIX: &str = "wtf.work";
    /// Signals subject: `wtf.signals.<namespace>.<instance_id>`
    pub const SIGNALS_PREFIX: &str = "wtf.signals";
    /// Archive subject: `wtf.archive.<namespace>.<instance_id>`
    pub const ARCHIVE_PREFIX: &str = "wtf.archive";

    /// Build a work queue subject for an activity type.
    #[must_use]
    pub fn work_subject(activity_type: &str) -> String {
        format!("{WORK_PREFIX}.{activity_type}")
    }

    /// Build a signals subject for an instance.
    #[must_use]
    pub fn signals_subject(namespace: &str, instance_id: &str) -> String {
        format!("{SIGNALS_PREFIX}.{namespace}.{instance_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_subject_format() {
        assert_eq!(subjects::work_subject("fetch_data"), "wtf.work.fetch_data");
    }

    #[test]
    fn work_subject_with_hyphen() {
        assert_eq!(subjects::work_subject("send-email"), "wtf.work.send-email");
    }

    #[test]
    fn signals_subject_format() {
        assert_eq!(
            subjects::signals_subject("payments", "01ARZ"),
            "wtf.signals.payments.01ARZ"
        );
    }

    #[test]
    fn stream_names_are_stable() {
        // These names are written to NATS — changing them is a breaking change.
        assert_eq!(stream_names::EVENTS, "wtf-events");
        assert_eq!(stream_names::WORK, "wtf-work");
        assert_eq!(stream_names::SIGNALS, "wtf-signals");
        assert_eq!(stream_names::ARCHIVE, "wtf-archive");
    }

    #[test]
    fn subject_prefixes_are_stable() {
        // Prefix changes break existing subscriptions.
        assert_eq!(subjects::EVENTS_PREFIX, "wtf.log");
        assert_eq!(subjects::WORK_PREFIX, "wtf.work");
    }

    // provision_streams and verify_streams require live NATS — covered by integration tests.
}
