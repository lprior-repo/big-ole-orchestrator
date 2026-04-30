use async_trait::async_trait;

use crate::message_router::{
    ActorDestination, ChannelEntry, ChannelId, DeadLetterEntry, RouteError, RouterConfig,
    TypedMessage,
};

use crate::port::MessageRouterPort;

use super::AsyncMessageRouter;

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
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route(channel_id, message).await
    }

    async fn route_unicast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route_unicast(channel_id, message).await
    }

    async fn route_broadcast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
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
