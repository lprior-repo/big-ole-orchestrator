use std::sync::{Arc, Mutex};

use super::backpressure::BackpressureSignal;
use super::budget::WriteBudget;
use super::entries::{AppendEntry, BlobWrite, ControlPlaneWrite, ProjectionWrite};
use super::queue::{BudgetQueuesError, QueueConfig, QueueStats};

pub struct Appender {
    queues: super::queue::BudgetQueues<AppendEntry>,
}

impl Appender {
    pub fn new(config: &QueueConfig, budget: WriteBudget) -> Self {
        Self {
            queues: super::queue::BudgetQueues::new(config, budget),
        }
    }

    #[must_use]
    pub fn stats(&self) -> Arc<Mutex<QueueStats>> {
        self.queues.stats()
    }

    #[must_use]
    pub const fn budget(&self) -> &WriteBudget {
        self.queues.budget()
    }

    #[must_use]
    pub const fn backpressure(&self) -> &Arc<BackpressureSignal> {
        self.queues.backpressure()
    }

    pub fn append_control_plane(&self, write: ControlPlaneWrite) -> Result<(), BudgetQueuesError> {
        self.queues.try_enqueue(&AppendEntry::ControlPlane(write))
    }

    pub fn append_projection(&self, write: ProjectionWrite) -> Result<(), BudgetQueuesError> {
        self.queues.try_enqueue(&AppendEntry::Projection(write))
    }

    pub fn append_blob(&self, write: BlobWrite) -> Result<(), BudgetQueuesError> {
        self.queues.try_enqueue(&AppendEntry::Blob(write))
    }

    pub fn dequeue_critical(&self) -> Option<AppendEntry> {
        self.queues
            .dequeue(super::write_class::WriteClass::CriticalControlPlane)
    }

    pub fn dequeue_projection(&self) -> Option<AppendEntry> {
        self.queues
            .dequeue(super::write_class::WriteClass::OperatorProjection)
    }

    pub fn dequeue_blob(&self) -> Option<AppendEntry> {
        self.queues
            .dequeue(super::write_class::WriteClass::BulkBlob)
    }
}
