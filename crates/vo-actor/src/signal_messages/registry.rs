use vo_types::InstanceId;

use super::signal_in::SignalName;
use super::signal_out::{AcceptResumeOutcome, CancelRequested, ContinueAsNewError, InstanceResumed,
    SignalAccepted, WorkflowCancelled, WorkflowContinued};
use super::signal_in::LifecycleState;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalStorageError {
    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("Write error for {instance_id}: {reason}")]
    WriteError {
        instance_id: InstanceId,
        reason: String,
    },
    #[error("Delete error for {instance_id}: {reason}")]
    DeleteError {
        instance_id: InstanceId,
        reason: String,
    },
}

pub trait SignalStorage: Send + Sync {
    fn persist_signal_accepted(&self, accepted: &SignalAccepted) -> Result<(), SignalStorageError>;

    fn remove_signal_accepted(
        &self,
        instance_id: &InstanceId,
        signal_id: &str,
    ) -> Result<(), SignalStorageError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalWorkQueueError {
    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("Enqueue error for {instance_id}: {reason}")]
    EnqueueError {
        instance_id: InstanceId,
        reason: String,
    },
}

pub trait SignalWorkQueue: Send + Sync {
    fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SignalWorkQueueError>;
}

pub mod mock_signal_storage {
    use super::*;

    #[derive(Debug, Default)]
    pub struct MockSignalStorage {
        persisted: std::sync::Mutex<Vec<SignalAccepted>>,
        should_fail: std::sync::Mutex<bool>,
    }

    impl MockSignalStorage {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        pub fn persisted_signals(&self) -> Vec<SignalAccepted> {
            self.persisted.lock().unwrap().clone()
        }

        #[allow(dead_code)]
        pub fn clear(&self) {
            self.persisted.lock().unwrap().clear();
        }
    }

    impl SignalStorage for MockSignalStorage {
        fn persist_signal_accepted(
            &self,
            accepted: &SignalAccepted,
        ) -> Result<(), SignalStorageError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SignalStorageError::WriteError {
                    instance_id: accepted.instance_id.clone(),
                    reason: "Mock storage failure".to_string(),
                });
            }
            self.persisted.lock().unwrap().push(accepted.clone());
            Ok(())
        }

        fn remove_signal_accepted(
            &self,
            instance_id: &InstanceId,
            signal_id: &str,
        ) -> Result<(), SignalStorageError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SignalStorageError::DeleteError {
                    instance_id: instance_id.clone(),
                    reason: "Mock storage failure".to_string(),
                });
            }
            let mut persisted = self.persisted.lock().unwrap();
            persisted
                .retain(|s| !(s.instance_id == *instance_id && s.signal_id.as_str() == signal_id));
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    pub struct MockSignalWorkQueue {
        enqueued: std::sync::Mutex<Vec<InstanceId>>,
        should_fail: std::sync::Mutex<bool>,
        instance_not_found: std::sync::Mutex<bool>,
    }

    impl MockSignalWorkQueue {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        pub fn set_instance_not_found(&self, not_found: bool) {
            *self.instance_not_found.lock().unwrap() = not_found;
        }

        pub fn enqueued_instances(&self) -> Vec<InstanceId> {
            self.enqueued.lock().unwrap().clone()
        }

        #[allow(dead_code)]
        pub fn clear(&self) {
            self.enqueued.lock().unwrap().clear();
        }
    }

    impl SignalWorkQueue for MockSignalWorkQueue {
        fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SignalWorkQueueError> {
            if *self.instance_not_found.lock().unwrap() {
                return Err(SignalWorkQueueError::InstanceNotFound(instance_id));
            }
            if *self.should_fail.lock().unwrap() {
                return Err(SignalWorkQueueError::EnqueueError {
                    instance_id,
                    reason: "Mock queue failure".to_string(),
                });
            }
            self.enqueued.lock().unwrap().push(instance_id);
            Ok(())
        }
    }
}