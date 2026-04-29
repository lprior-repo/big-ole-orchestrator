//! Port trait for Actor message router.
//!
//! Defines the interface for distributed message routing.
//! Implementors must be Send + Sync.

use async_trait::async_trait;

use crate::message_router::{
    ActorDestination, ChannelEntry, ChannelId, DeadLetterEntry, RouteError, RouterConfig,
    TypedMessage,
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
        message: TypedMessage<T>,
    ) -> Result<(), RouteError>;

    async fn route_unicast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError>;

    async fn route_broadcast<T: Send + Sync + 'static>(
        &self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_message_router::AsyncMessageRouter;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn message_router_port_is_send() {
        assert_send::<dyn MessageRouterPort>();
    }

    #[test]
    fn message_router_port_is_sync() {
        assert_sync::<dyn MessageRouterPort>();
    }

    #[test]
    fn async_message_router_implements_port() {
        fn implements_port<T: MessageRouterPort>() {}
        implements_port::<AsyncMessageRouter>();
    }

    fn test_channel_id() -> ChannelId {
        ChannelId::new("test-channel")
    }

    fn test_destination() -> ActorDestination {
        ActorDestination::new(String::from("test-actor"))
    }

    #[tokio::test]
    async fn port_register_channel_returns_ok() {
        let router = AsyncMessageRouter::with_default_config();
        let result = router
            .register_channel(test_channel_id(), test_destination())
            .await;
        assert!(result.is_ok(), "register_channel should succeed");
    }

    #[tokio::test]
    async fn port_register_channel_idempotent_on_duplicate() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();
        let result = router
            .register_channel(channel_id, test_destination())
            .await;
        assert!(
            result.is_err(),
            "duplicate channel registration should fail"
        );
    }

    #[tokio::test]
    async fn port_has_channel_reflects_registration() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        assert!(!router.has_channel(&channel_id).await);
        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();
        assert!(router.has_channel(&channel_id).await);
    }

    #[tokio::test]
    async fn port_num_channels_reflects_registration() {
        let router = AsyncMessageRouter::with_default_config();
        assert_eq!(router.num_channels().await, 0);
        router
            .register_channel(test_channel_id(), test_destination())
            .await
            .unwrap();
        assert_eq!(router.num_channels().await, 1);
    }

    #[tokio::test]
    async fn port_unregister_channel_removes_channel() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();
        let removed = router.unregister_channel(&channel_id).await;
        assert!(removed.is_some(), "unregister_channel should return Some");
        assert!(!router.has_channel(&channel_id).await);
    }

    #[tokio::test]
    async fn port_add_destination_increments_total() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();
        assert_eq!(router.total_destinations().await, 1);
        router
            .add_destination(&channel_id, test_destination())
            .await
            .unwrap();
        assert_eq!(router.total_destinations().await, 2);
    }

    #[tokio::test]
    async fn port_deactivate_channel_marks_inactive() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();
        assert!(router.is_channel_active(&channel_id).await);
        router.deactivate_channel(&channel_id).await.unwrap();
        assert!(!router.is_channel_active(&channel_id).await);
    }

    #[tokio::test]
    async fn port_config_returns_router_config() {
        let router = AsyncMessageRouter::with_default_config();
        let config = router.config().await;
        assert_eq!(config.max_destinations_per_channel, 16);
        assert_eq!(config.max_dlq_size, 1000);
    }

    #[tokio::test]
    async fn port_total_active_destinations_counts_active_only() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        router
            .register_channel(channel_id.clone(), test_destination())
            .await
            .unwrap();
        router
            .add_destination(&channel_id, test_destination())
            .await
            .unwrap();
        assert_eq!(router.total_active_destinations().await, 2);
        router.deactivate_channel(&channel_id).await.unwrap();
        assert_eq!(router.total_active_destinations().await, 0);
    }

    #[tokio::test]
    async fn port_dlq_depth_starts_at_zero() {
        let router = AsyncMessageRouter::with_default_config();
        assert_eq!(router.dlq_depth().await, 0);
    }

    #[tokio::test]
    async fn port_clear_dlq_works() {
        let router = AsyncMessageRouter::with_default_config();
        router.clear_dlq().await;
        assert_eq!(router.dlq_depth().await, 0);
    }

    #[tokio::test]
    async fn port_register_broadcast_channel_creates_channel() {
        let router = AsyncMessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let result = router
            .register_broadcast_channel(
                channel_id.clone(),
                vec![test_destination(), test_destination()],
            )
            .await;
        assert!(result.is_ok());
        assert!(router.has_channel(&channel_id).await);
    }
}
