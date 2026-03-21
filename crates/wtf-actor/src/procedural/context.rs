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
        ActivityId::procedural(&self.instance_id, self.op_counter.load(Ordering::SeqCst))
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
}
