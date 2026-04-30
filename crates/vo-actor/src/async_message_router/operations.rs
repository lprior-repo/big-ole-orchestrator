use crate::message_router::{ChannelId, RouteError, TypedMessage};

use super::AsyncMessageRouter;

impl AsyncMessageRouter {
    /// Route a message to all destinations in the channel (send operation).
    pub async fn route<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route(channel_id, message).await
    }

    /// Route a message to a single destination in the channel (unicast operation).
    pub async fn route_unicast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route_unicast(channel_id, message).await
    }

    /// Route a message to all destinations in the channel (broadcast operation).
    pub async fn route_broadcast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        let mut router = self.inner.lock().await;
        router.route_broadcast(channel_id, message).await
    }
}
