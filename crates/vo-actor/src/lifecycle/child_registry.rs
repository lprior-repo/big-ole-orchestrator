//! Parent-child relationship tracking for hierarchical lifecycle.
//!
//! Maintains the actor hierarchy and tracks child states for graceful
//! shutdown propagation.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use vo_types::InstanceId;

use super::state::ActorLifecycleState;

// =============================================================================
// Parent-Child Relationships
// =============================================================================

/// Information about a child actor.
#[derive(Debug, Clone)]
pub struct ChildInfo {
    pub instance_id: InstanceId,
    pub state: ActorLifecycleState,
    pub added_at: std::time::Instant,
}

/// Parent-child relationship tracker for hierarchical lifecycle.
///
/// Maintains the actor hierarchy and tracks child states for
/// graceful shutdown propagation.
#[derive(Debug)]
pub struct ParentChildRegistry {
    children: Arc<RwLock<HashMap<InstanceId, ChildInfo>>>,
}

impl Default for ParentChildRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentChildRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Adds a child to this parent's registry.
    pub async fn add_child(&self, instance_id: InstanceId) {
        let mut children = self.children.write().await;
        let info = ChildInfo {
            instance_id: instance_id.clone(),
            state: ActorLifecycleState::Pending,
            added_at: std::time::Instant::now(),
        };
        children.insert(instance_id, info);
    }

    /// Removes a child from this parent's registry.
    pub async fn remove_child(&self, instance_id: &InstanceId) {
        let mut children = self.children.write().await;
        children.remove(instance_id);
    }

    /// Updates a child's lifecycle state.
    pub async fn update_child_state(
        &self,
        instance_id: &InstanceId,
        new_state: ActorLifecycleState,
    ) -> Option<ChildInfo> {
        let mut children = self.children.write().await;
        children.get_mut(instance_id).map(|info| {
            info.state = new_state;
            info.clone()
        })
    }

    /// Gets all children with their current states.
    pub async fn get_children(&self) -> HashMap<InstanceId, ChildInfo> {
        let children = self.children.read().await;
        children.clone()
    }

    /// Gets children in a specific state.
    pub async fn get_children_by_state(&self, state: ActorLifecycleState) -> Vec<InstanceId> {
        let children = self.children.read().await;
        children
            .iter()
            .filter(|(_, info)| info.state == state)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Returns true if all children are in terminal state.
    pub async fn all_children_terminal(&self) -> bool {
        let children = self.children.read().await;
        children.values().all(|info| info.state.is_terminal())
    }

    /// Returns true if all children have stopped (including Failed).
    pub async fn all_children_stopped(&self) -> bool {
        let children = self.children.read().await;
        children.values().all(|info| info.state.is_stopping())
    }

    /// Returns the count of non-terminal children.
    pub async fn active_children_count(&self) -> usize {
        let children = self.children.read().await;
        children
            .values()
            .filter(|info| !info.state.is_terminal())
            .count()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::state::ActorLifecycleState;
    use super::*;

    #[tokio::test]
    async fn add_remove() {
        let registry = ParentChildRegistry::new();
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        registry.add_child(id.clone()).await;
        let children = registry.get_children().await;
        assert!(children.contains_key(&id));

        registry.remove_child(&id).await;
        let children = registry.get_children().await;
        assert!(!children.contains_key(&id));
    }

    #[tokio::test]
    async fn update_state() {
        let registry = ParentChildRegistry::new();
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        registry.add_child(id.clone()).await;
        let info = registry
            .update_child_state(&id, ActorLifecycleState::Running)
            .await;
        assert_eq!(info.unwrap().state, ActorLifecycleState::Running);
    }

    #[tokio::test]
    async fn get_children_by_state() {
        let registry = ParentChildRegistry::new();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        registry.add_child(id1.clone()).await;
        registry.add_child(id2.clone()).await;
        registry
            .update_child_state(&id1, ActorLifecycleState::Running)
            .await;

        let running = registry
            .get_children_by_state(ActorLifecycleState::Running)
            .await;
        assert_eq!(running, vec![id1]);

        let pending = registry
            .get_children_by_state(ActorLifecycleState::Pending)
            .await;
        assert_eq!(pending, vec![id2]);
    }

    #[tokio::test]
    async fn all_children_terminal() {
        let registry = ParentChildRegistry::new();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        registry.add_child(id1.clone()).await;
        registry.add_child(id2.clone()).await;
        registry
            .update_child_state(&id1, ActorLifecycleState::Stopped)
            .await;

        assert!(!registry.all_children_terminal().await);

        registry
            .update_child_state(&id2, ActorLifecycleState::Failed)
            .await;

        assert!(registry.all_children_terminal().await);
    }

    #[tokio::test]
    async fn active_count() {
        let registry = ParentChildRegistry::new();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        registry.add_child(id1.clone()).await;
        registry.add_child(id2.clone()).await;
        assert_eq!(registry.active_children_count().await, 2);

        registry
            .update_child_state(&id1, ActorLifecycleState::Stopped)
            .await;
        assert_eq!(registry.active_children_count().await, 1);

        registry
            .update_child_state(&id2, ActorLifecycleState::Failed)
            .await;
        assert_eq!(registry.active_children_count().await, 0);
    }
}
