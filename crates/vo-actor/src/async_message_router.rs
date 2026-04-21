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
        ActorDestination::new(String::from("test-actor"))
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
}
