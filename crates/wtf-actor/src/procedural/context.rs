//! Context passed to a procedural workflow function.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::messages::InstanceMsg;
use bytes::Bytes;
use ractor::ActorRef;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use wtf_common::{ActivityId, InstanceId};

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
    pub fn new(
        instance_id: InstanceId,
        initial_op_counter: u32,
        myself: ActorRef<InstanceMsg>,
    ) -> Self {
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
                |reply| InstanceMsg::ProceduralSleep {
                    operation_id: op_id,
                    duration,
                    reply,
                },
                None,
            )
            .await?;

        match result {
            ractor::rpc::CallResult::Success(r) => r?,
            _ => anyhow::bail!("Actor call failed"),
        }

        self.op_counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Sample the current UTC time deterministically.
    pub async fn now(&self) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
        let operation_id = self.op_counter.load(Ordering::SeqCst);
        let result = self
            .myself
            .call(
                |reply| crate::messages::InstanceMsg::ProceduralNow {
                    operation_id,
                    reply,
                },
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
        let operation_id = self.op_counter.load(Ordering::SeqCst);
        let result = self
            .myself
            .call(
                |reply| crate::messages::InstanceMsg::ProceduralRandom {
                    operation_id,
                    reply,
                },
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

    /// Block until a signal with the given name is received.
    ///
    /// Follows the same dual-phase pattern as `activity()` and `sleep()`:
    /// 1. Check for a checkpoint (replay) → return result immediately.
    /// 2. Otherwise dispatch `ProceduralWaitForSignal` and await reply.
    pub async fn wait_for_signal(&self, signal_name: &str) -> anyhow::Result<Bytes> {
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

        // 2. Dispatch wait and await signal delivery (Live logic)
        let result = self
            .myself
            .call(
                |reply| InstanceMsg::ProceduralWaitForSignal {
                    operation_id: op_id,
                    signal_name: signal_name.to_owned(),
                    reply,
                },
                None,
            )
            .await?;

        match result {
            ractor::rpc::CallResult::Success(r) => {
                self.op_counter.fetch_add(1, Ordering::SeqCst);
                r.map_err(anyhow::Error::from)
            }
            _ => anyhow::bail!("Actor call failed"),
        }
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
