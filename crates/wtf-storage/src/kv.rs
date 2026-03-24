//! NATS KV bucket provisioning and operations — materialized view layer (ADR-014).
//!
//! The KV stores are the QUERY side of the CQRS split. They are derived from `JetStream`
//! and can be fully reconstructed from the event log (`wtf admin rebuild-views`).
//! They are NEVER the source of truth.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use async_nats::jetstream::{
    kv::{Config as KvConfig, Store},
    Context,
};
use wtf_common::{InstanceId, WtfError};

/// Handles to all five KV buckets used by wtf-engine (ADR-014).
#[derive(Clone)]
pub struct KvStores {
    /// `wtf-instances`: current status of each workflow instance. Key: `<ns>/<id>`.
    pub instances: Store,
    /// `wtf-timers`: pending timers. Key: `<timer_id>`. Deleted on fire or cancel.
    pub timers: Store,
    /// `wtf-definitions`: workflow type definitions. Key: `<ns>/<workflow_type>`.
    pub definitions: Store,
    /// `wtf-heartbeats`: engine node liveness. Key: `hb/<instance_id>`. TTL=10s.
    pub heartbeats: Store,
}

/// Provision all five KV buckets. Idempotent — safe to call on every startup.
///
/// # Errors
/// Returns [`WtfError::NatsPublish`] if any bucket creation fails.
pub async fn provision_kv_buckets(js: &Context) -> Result<KvStores, WtfError> {
    Ok(KvStores {
        instances: provision_instances_kv(js).await?,
        timers: provision_timers_kv(js).await?,
        definitions: provision_definitions_kv(js).await?,
        heartbeats: provision_heartbeats_kv(js).await?,
    })
}

async fn provision_instances_kv(js: &Context) -> Result<Store, WtfError> {
    create_or_get_kv(
        js,
        KvConfig {
            bucket: bucket_names::INSTANCES.to_owned(),
            description: "Current status of workflow instances".to_owned(),
            num_replicas: 1,
            storage: async_nats::jetstream::stream::StorageType::File,
            history: 1,
            ..Default::default()
        },
    )
    .await
}

async fn provision_timers_kv(js: &Context) -> Result<Store, WtfError> {
    create_or_get_kv(
        js,
        KvConfig {
            bucket: bucket_names::TIMERS.to_owned(),
            description: "Pending workflow timers".to_owned(),
            num_replicas: 1,
            storage: async_nats::jetstream::stream::StorageType::File,
            history: 1,
            ..Default::default()
        },
    )
    .await
}

async fn provision_definitions_kv(js: &Context) -> Result<Store, WtfError> {
    create_or_get_kv(
        js,
        KvConfig {
            bucket: bucket_names::DEFINITIONS.to_owned(),
            description: "Workflow type definitions".to_owned(),
            num_replicas: 1,
            storage: async_nats::jetstream::stream::StorageType::File,
            history: 5,
            ..Default::default()
        },
    )
    .await
}

async fn provision_heartbeats_kv(js: &Context) -> Result<Store, WtfError> {
    create_or_get_kv(
        js,
        KvConfig {
            bucket: bucket_names::HEARTBEATS.to_owned(),
            description: "Engine actor heartbeats — expiry triggers crash recovery".to_owned(),
            num_replicas: 1,
            storage: async_nats::jetstream::stream::StorageType::Memory,
            history: 1,
            max_age: Duration::from_secs(10),
            ..Default::default()
        },
    )
    .await
}

async fn create_or_get_kv(js: &Context, config: KvConfig) -> Result<Store, WtfError> {
    js.create_key_value(config.clone())
        .await
        .map_err(|e| WtfError::nats_publish(format!("create KV bucket {}: {e}", config.bucket)))
}

/// Write the heartbeat for an instance. Must be called every <5s to avoid expiry.
///
/// # Errors
/// Returns [`WtfError::NatsPublish`] on KV write failure.
pub async fn write_heartbeat(
    heartbeats: &Store,
    instance_id: &InstanceId,
    engine_node_id: &str,
) -> Result<(), WtfError> {
    let key = heartbeat_key(instance_id);
    heartbeats
        .put(&key, engine_node_id.as_bytes().to_vec().into())
        .await
        .map_err(|e| WtfError::nats_publish(format!("write heartbeat {instance_id}: {e}")))?;
    Ok(())
}

/// Delete the heartbeat for an instance (called when instance completes or is evicted).
///
/// # Errors
/// Returns [`WtfError::NatsPublish`] on KV delete failure.
pub async fn delete_heartbeat(
    heartbeats: &Store,
    instance_id: &InstanceId,
) -> Result<(), WtfError> {
    let key = heartbeat_key(instance_id);
    heartbeats
        .delete(&key)
        .await
        .map_err(|e| WtfError::nats_publish(format!("delete heartbeat {instance_id}: {e}")))?;
    Ok(())
}

/// Build the KV key for a heartbeat entry.
#[must_use]
pub fn heartbeat_key(instance_id: &InstanceId) -> String {
    format!("hb/{}", instance_id.as_str())
}

/// Build the KV key for an instance view entry.
#[must_use]
pub fn instance_key(namespace: &str, instance_id: &InstanceId) -> String {
    format!("{}/{}", namespace, instance_id.as_str())
}

/// Build the KV key for a workflow definition.
#[must_use]
pub fn definition_key(namespace: &str, workflow_type: &str) -> String {
    format!("{namespace}/{workflow_type}")
}

/// KV bucket names — stable constants (changing breaks existing deployments).
pub mod bucket_names {
    pub const INSTANCES: &str = "wtf-instances";
    pub const TIMERS: &str = "wtf-timers";
    pub const DEFINITIONS: &str = "wtf-definitions";
    pub const HEARTBEATS: &str = "wtf-heartbeats";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_key_format() {
        let id = InstanceId::new("01ARZ");
        assert_eq!(heartbeat_key(&id), "hb/01ARZ");
    }

    #[test]
    fn instance_key_format() {
        let id = InstanceId::new("01ARZ");
        assert_eq!(instance_key("payments", &id), "payments/01ARZ");
    }

    #[test]
    fn definition_key_format() {
        assert_eq!(definition_key("payments", "checkout"), "payments/checkout");
    }

    #[test]
    fn bucket_names_are_stable() {
        assert_eq!(bucket_names::INSTANCES, "wtf-instances");
        assert_eq!(bucket_names::TIMERS, "wtf-timers");
        assert_eq!(bucket_names::DEFINITIONS, "wtf-definitions");
        assert_eq!(bucket_names::HEARTBEATS, "wtf-heartbeats");
    }

    #[test]
    fn heartbeat_key_uses_hb_prefix() {
        let id = InstanceId::new("abc");
        assert!(heartbeat_key(&id).starts_with("hb/"));
    }

    // provision_kv_buckets, write_heartbeat, delete_heartbeat require live NATS — integration tests.
}
