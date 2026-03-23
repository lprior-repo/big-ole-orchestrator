use super::reporting::send_heartbeat;
use async_nats::jetstream::Context;
use std::sync::Arc;
use wtf_common::{ActivityId, InstanceId, NamespaceId, WtfError};

/// A handle for sending heartbeats during activity execution.
#[derive(Debug, Clone)]
pub struct HeartbeatSender {
    js: Context,
    namespace: NamespaceId,
    instance_id: InstanceId,
    activity_id: ActivityId,
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

impl HeartbeatSender {
    /// Create a new heartbeat sender for the given activity.
    #[must_use]
    pub fn new(
        js: Context,
        namespace: NamespaceId,
        instance_id: InstanceId,
        activity_id: ActivityId,
    ) -> Self {
        Self {
            js,
            namespace,
            instance_id,
            activity_id,
            stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Send a heartbeat with the given progress message.
    pub async fn send(&self, progress: &str) -> Result<u64, WtfError> {
        if self.stopped.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(WtfError::HeartbeatStopped);
        }

        send_heartbeat(
            &self.js,
            &self.namespace,
            &self.instance_id,
            &self.activity_id,
            progress,
        )
        .await
    }

    /// Stop sending heartbeats and release resources.
    pub fn stop(&self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
