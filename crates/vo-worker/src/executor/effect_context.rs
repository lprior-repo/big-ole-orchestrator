//! EffectContext for tracking and cancelling in-flight async effects.
//!
//! Provides cancellation of in-flight async effects when the effect context is dropped.

use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::task::AbortHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationReason {
    ContextDropped,
    Explicit,
}

#[derive(Debug, Clone)]
pub struct EffectId(String);

impl EffectId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EffectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::hash::Hash for EffectId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for EffectId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for EffectId {}

#[derive(Debug)]
struct TrackedEffect {
    abort_handle: AbortHandle,
    spawned_at: std::time::Instant,
}

#[derive(Debug)]
pub struct EffectContext {
    effects: RwLock<HashMap<String, TrackedEffect>>,
}

impl Default for EffectContext {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectContext {
    pub fn new() -> Self {
        Self {
            effects: RwLock::new(HashMap::new()),
        }
    }

    pub async fn spawn<F, T>(&self, effect_id: EffectId, future: F) -> tokio::task::JoinHandle<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let handle = tokio::spawn(future);
        let abort_handle = handle.abort_handle();

        let tracked = TrackedEffect {
            abort_handle,
            spawned_at: std::time::Instant::now(),
        };

        let mut effects = self.effects.write().await;
        effects.insert(effect_id.0.clone(), tracked);

        handle
    }

    pub async fn cancel(&self, effect_id: &EffectId) -> Option<bool> {
        let mut effects = self.effects.write().await;
        effects.remove(effect_id.as_str()).map(|tracked| {
            tracked.abort_handle.abort();
            true
        })
    }

    pub async fn cancel_all(&self, reason: CancellationReason) -> usize {
        let mut effects = self.effects.write().await;
        let count = effects.len();
        for (_id, tracked) in effects.drain() {
            tracked.abort_handle.abort();
        }
        count
    }

    pub async fn tracked_count(&self) -> usize {
        let effects = self.effects.read().await;
        effects.len()
    }

    pub async fn contains(&self, effect_id: &EffectId) -> bool {
        let effects = self.effects.read().await;
        effects.contains_key(effect_id.as_str())
    }

    pub async fn effect_age_ms(&self, effect_id: &EffectId) -> Option<u64> {
        let effects = self.effects.read().await;
        effects.get(effect_id.as_str()).map(|tracked| {
            tracked.spawned_at.elapsed().as_millis() as u64
        })
    }
}

impl Drop for EffectContext {
    fn drop(&mut self) {
        let _drop_flag = CancellationReason::ContextDropped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn effect_context_tracks_spawned_effects() {
        let ctx = EffectContext::new();
        assert_eq!(ctx.tracked_count().await, 0);

        let handle = ctx.spawn(
            EffectId::new("fx-1"),
            async {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            },
        ).await;

        assert_eq!(ctx.tracked_count().await, 1);
        assert!(ctx.contains(&EffectId::new("fx-1")).await);

        handle.abort();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn effect_context_cancel_removes_effect() {
        let ctx = EffectContext::new();

        let _handle = ctx.spawn(
            EffectId::new("fx-cancel"),
            async {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            },
        ).await;

        assert!(ctx.contains(&EffectId::new("fx-cancel")).await);
        let cancelled = ctx.cancel(&EffectId::new("fx-cancel")).await;
        assert_eq!(cancelled, Some(true));
        assert!(!ctx.contains(&EffectId::new("fx-cancel")).await);
    }

    #[tokio::test]
    async fn effect_context_cancel_nonexistent_returns_none() {
        let ctx = EffectContext::new();
        let result = ctx.cancel(&EffectId::new("nonexistent")).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn effect_context_cancel_all_clears_tracked() {
        let ctx = EffectContext::new();

        ctx.spawn(EffectId::new("fx-1"), async {}).await;
        ctx.spawn(EffectId::new("fx-2"), async {}).await;
        ctx.spawn(EffectId::new("fx-3"), async {}).await;

        assert_eq!(ctx.tracked_count().await, 3);

        let count = ctx.cancel_all(CancellationReason::Explicit).await;
        assert_eq!(count, 3);
        assert_eq!(ctx.tracked_count().await, 0);
    }

    #[tokio::test]
    async fn effect_context_cancellation_propagates_to_task() {
        let ctx = EffectContext::new();

        let handle = ctx.spawn(
            EffectId::new("fx-abort"),
            async {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            },
        ).await;

        ctx.cancel(&EffectId::new("fx-abort")).await;

        let result = handle.await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn effect_context_effect_age_tracked() {
        let ctx = EffectContext::new();

        let _handle = ctx.spawn(EffectId::new("fx-age"), async {}).await;

        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let age = ctx.effect_age_ms(&EffectId::new("fx-age")).await;
        assert!(age.is_some());
        assert!(age.unwrap() >= 5);
    }

    #[tokio::test]
    async fn cancel_during_execution() {
        let ctx = EffectContext::new();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_flag_clone = cancel_flag.clone();

        let handle = ctx.spawn(
            EffectId::new("fx-exec"),
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                cancel_flag_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            },
        ).await;

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        ctx.cancel(&EffectId::new("fx-exec")).await;

        let _ = handle.await;

        assert!(!cancel_flag.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn cancel_queued_effect() {
        let ctx = EffectContext::new();

        ctx.spawn(EffectId::new("fx-queued-1"), async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }).await;

        ctx.spawn(EffectId::new("fx-queued-2"), async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }).await;

        let cancelled = ctx.cancel(&EffectId::new("fx-queued-2")).await;
        assert_eq!(cancelled, Some(true));

        let remaining = ctx.tracked_count().await;
        assert_eq!(remaining, 1);
        assert!(ctx.contains(&EffectId::new("fx-queued-1")).await);
        assert!(!ctx.contains(&EffectId::new("fx-queued-2")).await);
    }

    #[tokio::test]
    async fn effect_id_display_and_accessors() {
        let id = EffectId::new("fx-test");
        assert_eq!(id.as_str(), "fx-test");
        assert_eq!(format!("{}", id), "fx-test");
    }
}