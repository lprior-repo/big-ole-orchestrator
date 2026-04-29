use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::message_router::{
    ActorDestination, ChannelEntry, ChannelId, DeadLetterEntry, RouteError, RouterConfig,
};
use crate::port::MessageRouterPort;

#[derive(Debug, Clone)]
pub struct AsyncMessageRouter {
    inner: Arc<Mutex<crate::message_router::MessageRouter>>,
}

impl AsyncMessageRouter {
    #[must_use]
    pub fn new(config: RouterConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(crate::message_router::MessageRouter::new(
                config,
            ))),
        }
    }

    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(RouterConfig::default())
    }
}

#[async_trait]
impl MessageRouterPort for AsyncMessageRouter {
    async fn register_channel(
        &self,
        channel_id: ChannelId,
        destination: ActorDestination,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.register_channel(channel_id, destination)
    }

    async fn register_broadcast_channel(
        &self,
        channel_id: ChannelId,
        destinations: Vec<ActorDestination>,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.register_broadcast_channel(channel_id, destinations)
    }

    async fn add_destination(
        &self,
        channel_id: &ChannelId,
        destination: ActorDestination,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.add_destination(channel_id, destination)
    }

    async fn remove_destination(
        &self,
        channel_id: &ChannelId,
        index: usize,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.remove_destination(channel_id, index)
    }

    async fn unregister_channel(&self, channel_id: &ChannelId) -> Option<ChannelEntry> {
        let mut router = self.inner.lock().await;
        router.unregister_channel(channel_id)
    }

    async fn deactivate_channel(&self, channel_id: &ChannelId) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.deactivate_channel(channel_id)
    }

    async fn route<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route(channel_id, message).await
    }

    async fn route_unicast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route_unicast(channel_id, message).await
    }

    async fn route_broadcast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route_broadcast(channel_id, message).await
    }

    async fn num_channels(&self) -> usize {
        let router = self.inner.lock().await;
        router.num_channels()
    }

    async fn total_destinations(&self) -> usize {
        let router = self.inner.lock().await;
        router.total_destinations()
    }

    async fn total_active_destinations(&self) -> usize {
        let router = self.inner.lock().await;
        router.total_active_destinations()
    }

    async fn dlq_depth(&self) -> usize {
        let router = self.inner.lock().await;
        router.dlq_depth()
    }

    async fn has_channel(&self, channel_id: &ChannelId) -> bool {
        let router = self.inner.lock().await;
        router.has_channel(channel_id)
    }

    async fn is_channel_active(&self, channel_id: &ChannelId) -> bool {
        let router = self.inner.lock().await;
        router.is_channel_active(channel_id)
    }

    async fn config(&self) -> RouterConfig {
        let router = self.inner.lock().await;
        router.config()
    }

    async fn drain_dlq(&self) -> Vec<DeadLetterEntry> {
        let mut router = self.inner.lock().await;
        router.drain_dlq()
    }

    async fn clear_dlq(&self) {
        let mut router = self.inner.lock().await;
        router.clear_dlq();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_channel_id() -> ChannelId {
        ChannelId::new("test-channel")
    }

    fn test_destination() -> ActorDestination {
        ActorDestination::test()
    }

    #[tokio::test]
    async fn async_message_router_new_creates_empty_router() {
        let router = AsyncMessageRouter::with_default_config();
        assert_eq!(router.num_channels().await, 0);
        assert_eq!(router.total_destinations().await, 0);
        assert_eq!(router.dlq_depth().await, 0);
    }

    #[tokio::test]
    async fn register_channel_adds_channel_to_router() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .await
            .unwrap();

        assert_eq!(router.num_channels().await, 1);
        assert!(router.has_channel(&channel_id).await);
    }

    #[tokio::test]
    async fn register_channel_prevents_duplicate_channels() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let dest1 = test_destination();
        let dest2 = test_destination();

        router
            .register_channel(channel_id.clone(), dest1)
            .await
            .unwrap();
        let result = router.register_channel(channel_id.clone(), dest2).await;

        assert!(result.is_err());
        assert_eq!(router.num_channels().await, 1);
    }

    #[tokio::test]
    async fn unregister_channel_removes_channel() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .await
            .unwrap();
        let removed = router.unregister_channel(&channel_id).await;

        assert!(removed.is_some());
        assert_eq!(router.num_channels().await, 0);
        assert!(!router.has_channel(&channel_id).await);
    }

    #[tokio::test]
    async fn add_destination_increments_destinations() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let dest1 = test_destination();
        let dest2 = test_destination();

        router
            .register_channel(channel_id.clone(), dest1)
            .await
            .unwrap();
        router.add_destination(&channel_id, dest2).await.unwrap();

        assert_eq!(router.total_destinations().await, 2);
    }

    #[tokio::test]
    async fn add_destination_fails_for_unknown_channel() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        let result = router.add_destination(&channel_id, destination).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn deactivate_channel_deactivates_all_destinations() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .await
            .unwrap();
        router.deactivate_channel(&channel_id).await.unwrap();

        assert!(!router.is_channel_active(&channel_id).await);
    }

    #[tokio::test]
    async fn clone_returns_shared_router() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .await
            .unwrap();

        let router2 = router.clone();
        assert_eq!(router2.num_channels().await, 1);
        assert!(router2.has_channel(&channel_id).await);
    }

    #[tokio::test]
    async fn concurrent_access_works() {
        let router = Arc::new(AsyncMessageRouter::with_default_config());
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .await
            .unwrap();

        let router2 = router.clone();
        let router3 = router.clone();

        let handle1 = tokio::spawn(async move {
            router2
                .add_destination(&channel_id, test_destination())
                .await
        });
        let handle2 = tokio::spawn(async move { router3.num_channels().await });

        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.unwrap().is_ok());
        assert_eq!(result2.unwrap(), 1);
    }

    #[tokio::test]
    async fn concurrent_registration_is_safe() {
        let router = Arc::new(AsyncMessageRouter::with_default_config());
        let channel_ids: Vec<ChannelId> = (0..10)
            .map(|i| ChannelId::new(format!("channel-{}", i)))
            .collect();
        let destinations: Vec<ActorDestination> = (0..10)
            .map(|i| ActorDestination::new(format!("actor-{}", i)))
            .collect();

        let mut handles = Vec::new();
        for i in 0..10 {
            let router_clone = router.clone();
            let channel_id = channel_ids[i].clone();
            let destination = destinations[i].clone();
            handles.push(tokio::spawn(async move {
                router_clone.register_channel(channel_id, destination).await
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Concurrent registration should succeed");
        }

        assert_eq!(router.num_channels().await, 10);
    }

    #[tokio::test]
    async fn concurrent_add_destination_maintains_consistency() {
        let router = Arc::new(AsyncMessageRouter::with_default_config());
        let channel_id = test_channel_id();

        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();

        let mut handles = Vec::new();
        for i in 0..5 {
            let router_clone = router.clone();
            handles.push(tokio::spawn(async move {
                router_clone
                    .add_destination(&channel_id, test_destination())
                    .await
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Concurrent add_destination should succeed");
        }

        assert_eq!(router.total_destinations().await, 6);
    }

    #[tokio::test]
    async fn concurrent_route_operations_are_safe() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let router = Arc::new(AsyncMessageRouter::with_default_config());
        let channel_id = test_channel_id();
        let counter = Arc::new(AtomicUsize::new(0));

        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();

        let mut handles = Vec::new();
        for i in 0..20 {
            let router_clone = router.clone();
            let counter_clone = counter.clone();
            handles.push(tokio::spawn(async move {
                let result = router_clone
                    .route(&channel_id, format!("message-{}", i))
                    .await;
                if result.is_ok() {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        assert_eq!(
            counter.load(Ordering::SeqCst),
            20,
            "All 20 concurrent route operations should complete"
        );
    }

    #[tokio::test]
    async fn route_broadcast_reaches_all_destinations() {
        let router = Arc::new(AsyncMessageRouter::with_default_config());
        let channel_id = test_channel_id();

        let dest1 = ActorDestination::new(String::from("actor-1"));
        let dest2 = ActorDestination::new(String::from("actor-2"));
        let dest3 = ActorDestination::new(String::from("actor-3"));

        router
            .register_channel(channel_id.clone(), dest1)
            .await
            .unwrap();
        router.add_destination(&channel_id, dest2).await.unwrap();
        router.add_destination(&channel_id, dest3).await.unwrap();

        let result = router
            .route_broadcast(&channel_id, "broadcast-message")
            .await;

        assert!(
            result.is_ok(),
            "Broadcast should succeed with multiple destinations"
        );
        assert_eq!(router.total_destinations().await, 3);
    }
}
