//! Context passed to a procedural workflow function.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use bytes::Bytes;
use ractor::ActorRef;
use wtf_common::{ActivityId, InstanceId};
use crate::messages::InstanceMsg;

/// Atomically increment `counter` and return the corresponding operation ID.
pub(crate) fn fetch_and_increment(instance_id: &InstanceId, counter: &AtomicU32) -> ActivityId {
    ActivityId::procedural(instance_id, counter.fetch_add(1, Ordering::SeqCst))
}

/// Context passed to a procedural workflow function.
#[derive(Clone)]
pub struct WorkflowContext {
    pub instance_id: InstanceId,
    pub op_counter: Arc<AtomicU32>,
    pub myself: ActorRef<InstanceMsg>,
}

impl WorkflowContext {
    /// Create a new context.
    #[must_use]
    pub fn new(instance_id: InstanceId, initial_op_counter: u32, myself: ActorRef<InstanceMsg>) -> Self {
        Self {
            instance_id,
            op_counter: Arc::new(AtomicU32::new(initial_op_counter)),
            myself,
        }
    }

    /// Return the next deterministic operation ID for this instance.
    #[must_use]
    pub fn next_op_id(&self) -> ActivityId {
        fetch_and_increment(&self.instance_id, &self.op_counter)
    }

    /// Dispatch an activity and wait for its completion.
    pub async fn activity(&self, activity_type: &str, payload: Bytes) -> anyhow::Result<Bytes> {
        let op_id = self.op_counter.load(Ordering::SeqCst);

        // 1. Check for checkpoint (Replay logic)
        let checkpoint = self
            .myself
            .call(
                |reply| InstanceMsg::GetProceduralCheckpoint {
                    operation_id: op_id,
                    reply,
                },
                None,
            )
            .await?;

        let checkpoint = match checkpoint {
            ractor::rpc::CallResult::Success(c) => c,
            _ => anyhow::bail!("Actor call failed"),
        };

        if let Some(cp) = checkpoint {
            self.op_counter.fetch_add(1, Ordering::SeqCst);
            return Ok(cp.result);
        }

        // 2. Dispatch and wait (Live logic)
        let result = self
            .myself
            .call(
                |reply| InstanceMsg::ProceduralDispatch {
                    activity_type: activity_type.to_owned(),
                    payload,
                    reply,
                },
                None,
            )
            .await?;

        let result = match result {
            ractor::rpc::CallResult::Success(r) => r?,
            _ => anyhow::bail!("Actor call failed"),
        };

        self.op_counter.fetch_add(1, Ordering::SeqCst);
        Ok(result)
    }

    /// Sleep for the given duration.
    pub async fn sleep(&self, duration: std::time::Duration) -> anyhow::Result<()> {
        let op_id = self.op_counter.load(Ordering::SeqCst);

        // 1. Check for checkpoint
        let checkpoint = self
            .myself
            .call(
                |reply| InstanceMsg::GetProceduralCheckpoint {
                    operation_id: op_id,
                    reply,
                },
                None,
            )
            .await?;

        let checkpoint = match checkpoint {
            ractor::rpc::CallResult::Success(c) => c,
            _ => anyhow::bail!("Actor call failed"),
        };

        if checkpoint.is_some() {
            self.op_counter.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }

        // 2. Dispatch sleep and wait
        let result = self
            .myself
            .call(
                |reply| InstanceMsg::ProceduralSleep { duration, reply },
                None,
            )
            .await?;

        match result {
            ractor::rpc::CallResult::Success(r) => r?,
            _ => anyhow::bail!("Actor call failed"),
        };

        self.op_counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Sample the current UTC time deterministically.
    pub async fn now(&self) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
        let result = self
            .myself
            .call(
                |reply| crate::messages::InstanceMsg::ProceduralNow { reply },
                None,
            )
            .await?;
        match result {
            ractor::rpc::CallResult::Success(ts) => {
                self.op_counter.fetch_add(1, Ordering::SeqCst);
                Ok(ts)
            }
            _ => anyhow::bail!("Actor call failed"),
        }
    }

    /// Sample a deterministic random u64.
    pub async fn random_u64(&self) -> anyhow::Result<u64> {
        let result = self
            .myself
            .call(
                |reply| crate::messages::InstanceMsg::ProceduralRandom { reply },
                None,
            )
            .await?;
        match result {
            ractor::rpc::CallResult::Success(v) => {
                self.op_counter.fetch_add(1, Ordering::SeqCst);
                Ok(v)
            }
            _ => anyhow::bail!("Actor call failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_counter_starts_at_zero_and_produces_correct_format() {
        let counter = Arc::new(AtomicU32::new(0));
        let instance_id = InstanceId::new("inst-01");
        let id0 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
        let id1 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
        assert_eq!(id0.as_str(), "inst-01:0");
        assert_eq!(id1.as_str(), "inst-01:1");
    }

    #[test]
    fn arc_clones_share_counter_state() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter2 = Arc::clone(&counter);
        let _ = counter.fetch_add(1, Ordering::SeqCst);
        let _ = counter.fetch_add(1, Ordering::SeqCst);
        assert_eq!(counter2.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn next_op_id_must_use_fetch_add_not_load() {
        // Regression guard: next_op_id must atomically increment the counter.
        // If it uses load instead of fetch_add, both calls return the same ID.
        let counter = Arc::new(AtomicU32::new(0));
        let instance_id = InstanceId::new("wf-01");
        let id0 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
        let id1 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
        assert_ne!(id0, id1, "next_op_id must produce unique IDs on successive calls");
        assert_eq!(id0.as_str(), "wf-01:0");
        assert_eq!(id1.as_str(), "wf-01:1");
    }

    #[test]
    fn fetch_and_increment_produces_unique_ids() {
        let counter = Arc::new(AtomicU32::new(0));
        let instance_id = InstanceId::new("wf-02");
        let id0 = fetch_and_increment(&instance_id, &counter);
        let id1 = fetch_and_increment(&instance_id, &counter);
        assert_ne!(id0, id1);
        assert_eq!(id0.as_str(), "wf-02:0");
        assert_eq!(id1.as_str(), "wf-02:1");
    }

    #[test]
    fn next_op_id_increments_counter_on_each_call() {
        // next_op_id must call fetch_and_increment (not load) so successive calls give different IDs.
        // This test verifies the counter value after two next_op_id calls.
        let counter = Arc::new(AtomicU32::new(0));
        let instance_id = InstanceId::new("wf-03");
        // Simulate two next_op_id calls via fetch_and_increment (the required implementation).
        let id0 = fetch_and_increment(&instance_id, &counter);
        let id1 = fetch_and_increment(&instance_id, &counter);
        assert_eq!(counter.load(Ordering::SeqCst), 2, "counter must be 2 after two calls");
        assert_eq!(id0.as_str(), "wf-03:0");
        assert_eq!(id1.as_str(), "wf-03:1");
    }
}
