//! Action Layer — Per-Workflow Semaphore Map
//!
//! Provides per-workflow semaphore management for fine-grained concurrency control.

use std::sync::Arc;

use tokio::sync::Semaphore;

use vo_types::WorkflowName;

use crate::semaphore::types::DEFAULT_MAX_PER_WORKFLOW;

/// Per-workflow semaphore map for fine-grained concurrency control.
pub struct WorkflowSemaphoreMap {
    semaphores: std::sync::RwLock<std::collections::HashMap<WorkflowName, Arc<Semaphore>>>,
    max_per_workflow: usize,
}

impl std::fmt::Debug for WorkflowSemaphoreMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowSemaphoreMap")
            .field("max_per_workflow", &self.max_per_workflow)
            .finish()
    }
}

impl WorkflowSemaphoreMap {
    /// Creates a new workflow semaphore map.
    #[must_use]
    pub fn new(max_per_workflow: usize) -> Self {
        Self {
            semaphores: std::sync::RwLock::new(std::collections::HashMap::new()),
            max_per_workflow,
        }
    }

    /// Creates a new workflow semaphore map with default settings.
    #[must_use]
    pub fn with_default_limits() -> Self {
        Self::new(DEFAULT_MAX_PER_WORKFLOW)
    }

    /// Gets or creates a semaphore for the given workflow.
    ///
    /// # Panics
    ///
    /// Panics if the semaphore lock is poisoned (should not happen unless a thread
    /// panicked while holding the lock, indicating a serious programming error).
    #[allow(clippy::unwrap_used)] // Lock poisoning indicates unrecoverable programming error
    fn get_or_create(&self, workflow_name: &WorkflowName) -> Arc<Semaphore> {
        {
            let semaphores = self.semaphores.read().unwrap();
            if let Some(sem) = semaphores.get(workflow_name) {
                return Arc::clone(sem);
            }
        }

        let mut semaphores = self.semaphores.write().unwrap();
        if let Some(sem) = semaphores.get(workflow_name) {
            return Arc::clone(sem);
        }

        let sem = Arc::new(Semaphore::new(self.max_per_workflow));
        semaphores.insert(workflow_name.clone(), Arc::clone(&sem));
        sem
    }

    /// Returns a reference to the semaphore for a workflow.
    ///
    /// The semaphore is created if it doesn't exist.
    pub fn semaphore_for(&self, workflow_name: &WorkflowName) -> Arc<Semaphore> {
        self.get_or_create(workflow_name)
    }

    /// Returns the number of semaphores currently tracked.
    #[must_use]
    #[allow(clippy::unwrap_used)] // Lock poisoning indicates unrecoverable programming error
    pub fn len(&self) -> usize {
        self.semaphores.read().unwrap().len()
    }

    /// Returns true if no workflows are being tracked.
    #[must_use]
    #[allow(clippy::unwrap_used)] // Lock poisoning indicates unrecoverable programming error
    pub fn is_empty(&self) -> bool {
        self.semaphores.read().unwrap().is_empty()
    }

    /// Cleans up semaphores with no waiting tasks.
    ///
    /// This is a best-effort cleanup to prevent memory growth.
    pub fn cleanup_idle(&self) {
        let mut semaphores = self.semaphores.write().unwrap();
        semaphores.retain(|_, sem| sem.available_permits() < self.max_per_workflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn workflow_semaphore_map_creation() {
        let map = WorkflowSemaphoreMap::default();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[tokio::test]
    async fn workflow_semaphore_map_semaphore_access() {
        let map = WorkflowSemaphoreMap::default();
        let wf_name = WorkflowName::parse("test-workflow").unwrap();

        let sem = map.semaphore_for(&wf_name);
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);

        let sem2 = map.semaphore_for(&wf_name);
        assert_eq!(map.len(), 1);

        let permit = sem.try_acquire().ok();
        assert!(permit.is_some());
    }

    #[tokio::test]
    async fn workflow_semaphore_map_different_workflows() {
        let map = WorkflowSemaphoreMap::default();
        let wf_a = WorkflowName::parse("workflow-a").unwrap();
        let wf_b = WorkflowName::parse("workflow-b").unwrap();

        let sem_a1 = map.semaphore_for(&wf_a);
        let sem_a2 = map.semaphore_for(&wf_a);
        let sem_b = map.semaphore_for(&wf_b);

        assert!(Arc::ptr_eq(&sem_a1, &sem_a2));
        assert!(!Arc::ptr_eq(&sem_a1, &sem_b));
        assert_eq!(map.len(), 2);
    }

    #[tokio::test]
    async fn workflow_semaphore_map_respects_max() {
        let map = WorkflowSemaphoreMap::new(2);
        let wf = WorkflowName::parse("limited-workflow").unwrap();

        let sem = map.semaphore_for(&wf);

        let p1 = sem.try_acquire().ok();
        let p2 = sem.try_acquire().ok();
        let p3 = sem.try_acquire().ok();

        assert!(p1.is_some());
        assert!(p2.is_some());
        assert!(p3.is_none());
    }
}
