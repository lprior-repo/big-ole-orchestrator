//! wtf-storage — NATS `JetStream` event log, NATS KV materialized view, sled snapshot cache.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub mod db;
pub mod instances;
pub mod journal;
pub mod kv;
pub mod nats;
pub mod provision;
pub mod replay;
pub mod signals;
pub mod snapshots;
pub mod timers;

pub use journal::{append_event, build_subject};
pub use kv::{
    bucket_names, definition_key, delete_heartbeat, heartbeat_key, instance_key,
    provision_kv_buckets, write_heartbeat, KvStores,
};
pub use nats::{connect, NatsClient, NatsConfig};
pub use provision::{provision_streams, stream_names, subjects, verify_streams};
pub use replay::{
    create_replay_consumer, replay_events, replay_start_seq, ReplayConfig, ReplayConsumer,
};
pub use snapshots::{
    delete_snapshot, open_snapshot_db, read_snapshot, write_snapshot, SnapshotRecord,
};
pub use wtf_common::storage::{ReplayBatch, ReplayedEvent};

use async_trait::async_trait;
use bytes::Bytes;
use futures::TryStreamExt;
use wtf_common::storage::{EventStore, ReplayStream, StateStore, TaskQueue};
use wtf_common::{InstanceId, InstanceMetadata, NamespaceId, WorkflowEvent, WtfError};

#[async_trait]
impl EventStore for NatsClient {
    async fn publish(
        &self,
        ns: &NamespaceId,
        inst: &InstanceId,
        event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        journal::append_event(&self.jetstream, ns, inst, &event).await
    }

    async fn open_replay_stream(
        &self,
        ns: &NamespaceId,
        inst: &InstanceId,
        from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        let config = ReplayConfig {
            from_seq,
            ..Default::default()
        };
        let consumer = create_replay_consumer(&self.jetstream, ns, inst, &config).await?;
        Ok(Box::new(consumer))
    }
}

#[async_trait]
impl StateStore for NatsClient {
    async fn put_instance_metadata(&self, metadata: InstanceMetadata) -> Result<(), WtfError> {
        let kv = self
            .jetstream
            .get_key_value(bucket_names::INSTANCES)
            .await
            .map_err(|e| WtfError::nats_publish(format!("get instances KV: {e}")))?;

        let json = serde_json::to_vec(&metadata)
            .map_err(|e| WtfError::nats_publish(format!("serialize metadata: {e}")))?;

        let key = instance_key(metadata.namespace.as_str(), &metadata.instance_id);
        kv.put(&key, json.into())
            .await
            .map_err(|e| WtfError::nats_publish(format!("put metadata: {e}")))?;

        Ok(())
    }

    async fn get_instance_metadata(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<InstanceMetadata>, WtfError> {
        let kv = self
            .jetstream
            .get_key_value(bucket_names::INSTANCES)
            .await
            .map_err(|e| WtfError::nats_publish(format!("get instances KV: {e}")))?;

        let Some(key) = find_instance_metadata_key(&kv, instance_id).await? else {
            return Ok(None);
        };

        let entry = kv.get(&key).await.ok().flatten();

        if let Some(e) = entry {
            return Ok(serde_json::from_slice(&e).ok());
        }

        Ok(None)
    }

    async fn put_heartbeat(&self, node_id: &str, instance_id: &InstanceId) -> Result<(), WtfError> {
        let kv = self
            .jetstream
            .get_key_value(bucket_names::HEARTBEATS)
            .await
            .map_err(|e| WtfError::nats_publish(format!("get heartbeats KV: {e}")))?;

        kv.put(
            &heartbeat_key(instance_id),
            node_id.as_bytes().to_vec().into(),
        )
        .await
        .map_err(|e| WtfError::nats_publish(format!("put heartbeat: {e}")))?;

        Ok(())
    }

    async fn put_timer(&self, timer_id: &str, payload: Bytes) -> Result<(), WtfError> {
        let kv = self
            .jetstream
            .get_key_value(bucket_names::TIMERS)
            .await
            .map_err(|e| WtfError::nats_publish(format!("get timers KV: {e}")))?;

        kv.put(timer_id, payload)
            .await
            .map_err(|e| WtfError::nats_publish(format!("put timer {timer_id}: {e}")))?;

        Ok(())
    }
}

async fn find_instance_metadata_key(
    kv: &async_nats::jetstream::kv::Store,
    instance_id: &InstanceId,
) -> Result<Option<String>, WtfError> {
    let mut keys = kv
        .keys()
        .await
        .map_err(|e| WtfError::nats_publish(format!("list instance keys: {e}")))?;

    while let Some(key) = keys
        .try_next()
        .await
        .map_err(|e| WtfError::nats_publish(format!("read instance key: {e}")))?
    {
        if key.rsplit('/').next() == Some(instance_id.as_str()) {
            return Ok(Some(key));
        }
    }

    Ok(None)
}

#[async_trait]
impl TaskQueue for NatsClient {
    async fn dispatch(&self, activity_type: &str, payload: Bytes) -> Result<(), WtfError> {
        let subject = format!("wtf.work.{activity_type}");
        self.jetstream
            .publish(subject, payload)
            .await
            .map_err(|e| WtfError::nats_publish(format!("dispatch task failed: {e}")))?;
        Ok(())
    }
}
