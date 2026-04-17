//! Port trait for Actor message router.
//!
//! Defines the interface for distributed message routing.
//! Implementors must be Send + Sync.

use async_trait::async_trait;

use crate::message_router::{
    ActorDestination, ChannelEntry, ChannelId, DeadLetterEntry, RouteError, RouterConfig,
};

#[async_trait]
pub trait MessageRouterPort: Send + Sync {
    async fn register_channel(
        &self,
        channel_id: ChannelId,
        destination: ActorDestination,
    ) -> Result<(), RouteError>;

    async fn register_broadcast_channel(
        &self,
        channel_id: ChannelId,
        destinations: Vec<ActorDestination>,
    ) -> Result<(), RouteError>;

    async fn add_destination(
        &self,
        channel_id: &ChannelId,
        destination: ActorDestination,
    ) -> Result<(), RouteError>;

    async fn remove_destination(
        &self,
        channel_id: &ChannelId,
        index: usize,
    ) -> Result<(), RouteError>;

    async fn unregister_channel(&self, channel_id: &ChannelId) -> Option<ChannelEntry>;

    async fn deactivate_channel(&self, channel_id: &ChannelId) -> Result<(), RouteError>;

    async fn route<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError>;

    async fn route_unicast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError>;

    async fn route_broadcast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError>;

    async fn num_channels(&self) -> usize;

    async fn total_destinations(&self) -> usize;

    async fn total_active_destinations(&self) -> usize;

    async fn dlq_depth(&self) -> usize;

    async fn has_channel(&self, channel_id: &ChannelId) -> bool;

    async fn is_channel_active(&self, channel_id: &ChannelId) -> bool;

    async fn config(&self) -> RouterConfig;

    async fn drain_dlq(&self) -> Vec<DeadLetterEntry>;

    async fn clear_dlq(&self);
}
