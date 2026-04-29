//! Actor message router with typed channels, routing table, broadcast/fan-out, and dead letter queue.
//!
//! Architecture: Data → Calc → Actions + DLQ
//! - **data**: `ChannelId`, `RouterConfig`, `TypedMessage`, `MessageMetadata`, `TimestampMs`,
//!   `ActorDestination`, `RoutingDestination`, `ChannelEntry`
//! - **dlq**: `DeadLetterEntry`, `DeadLetterMessage`, `DeadLetterReason`, `DeadLetterQueue`
//! - **calc**: Pure routing decisions, destination resolution, `RouteError`
//! - **actions**: `MessageRouter` — message dispatch via actor channels
//!
//! # Example
//!
//! ```ignore
//! use vo_actor::message_router::{MessageRouter, RouterConfig, ChannelId};
//! use ractor::{Actor, ActorRef};
//!
//! let config = RouterConfig::default();
//! let router = MessageRouter::new(config);
//!
//! let channel_id = ChannelId::new("workflow-events");
//! router.register_channel(channel_id.clone(), actor_ref).unwrap();
//!
//! router.route(channel_id, message).await;
//! ```

pub mod actions;
pub mod calc;
pub mod data;
pub mod dlq;

pub use actions::MessageRouter;
pub use calc::RouteError;
pub use data::{
    ActorDestination, ChannelEntry, ChannelId, MessageMetadata, MessageSink, RouterConfig,
    RouterEnvelope, RoutingDestination, TimestampMs, TypedMessage,
};
pub use dlq::{DeadLetterEntry, DeadLetterMessage, DeadLetterQueue, DeadLetterReason};

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    fn test_channel_id() -> ChannelId {
        ChannelId::new("test-channel")
    }

    fn test_destination() -> ActorDestination {
        ActorDestination::test()
    }

    #[test]
    fn channel_id_new_creates_valid_id() {
        let id = ChannelId::new("my-channel");
        assert_eq!(id.as_str(), "my-channel");
    }

    #[test]
    fn channel_id_parse_rejects_empty() {
        let result = ChannelId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn router_config_default_has_sensible_values() {
        let config = RouterConfig::default();
        assert_eq!(config.max_destinations_per_channel, 16);
        assert_eq!(config.max_dlq_size, 1000);
        assert_eq!(config.delivery_timeout, Duration::from_secs(5));
        assert!(config.broadcast_enabled);
    }

    #[test]
    fn message_router_new_creates_empty_router() {
        let router = MessageRouter::with_default_config();
        assert_eq!(router.num_channels(), 0);
        assert_eq!(router.total_destinations(), 0);
        assert_eq!(router.dlq_depth(), 0);
    }

    #[test]
    fn register_channel_adds_channel_to_router() {
        let mut router = MessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .unwrap();

        assert_eq!(router.num_channels(), 1);
        assert!(router.has_channel(&channel_id));
    }

    #[test]
    fn register_channel_prevents_duplicate_channels() {
        let mut router = MessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let dest1 = test_destination();
        let dest2 = test_destination();

        router.register_channel(channel_id.clone(), dest1).unwrap();
        let result = router.register_channel(channel_id.clone(), dest2);

        assert!(result.is_err());
        assert_eq!(router.num_channels(), 1);
    }

    #[test]
    fn unregister_channel_removes_channel() {
        let mut router = MessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .unwrap();
        let removed = router.unregister_channel(&channel_id);

        assert!(removed.is_some());
        assert_eq!(router.num_channels(), 0);
        assert!(!router.has_channel(&channel_id));
    }

    #[test]
    fn add_destination_increments_destinations() {
        let mut router = MessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let dest1 = test_destination();
        let dest2 = test_destination();

        router.register_channel(channel_id.clone(), dest1).unwrap();
        router.add_destination(&channel_id, dest2).unwrap();

        assert_eq!(router.total_destinations(), 2);
    }

    #[test]
    fn add_destination_fails_for_unknown_channel() {
        let mut router = MessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        let result = router.add_destination(&channel_id, destination);

        assert!(result.is_err());
    }

    #[test]
    fn deactivate_channel_deactivates_all_destinations() {
        let mut router = MessageRouter::with_default_config();
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .unwrap();
        router.deactivate_channel(&channel_id).unwrap();

        assert!(!router.is_channel_active(&channel_id));
    }

    #[test]
    fn dead_letter_queue_fifo_eviction() {
        let mut dlq = DeadLetterQueue::new(3);

        for i in 0..5 {
            let entry = DeadLetterEntry {
                channel_id: ChannelId::new(format!("channel-{}", i)),
                message: DeadLetterMessage {
                    payload: vec![],
                    type_name: "test".to_string(),
                },
                enqueued_at: TimestampMs::now(),
                reason: DeadLetterReason::ChannelNotFound,
            };
            dlq.enqueue(entry);
        }

        assert_eq!(dlq.len(), 3);
        let entries: Vec<_> = dlq
            .entries()
            .iter()
            .map(|e| e.channel_id.as_str())
            .collect();
        assert!(entries.contains(&"channel-2"));
        assert!(entries.contains(&"channel-3"));
        assert!(entries.contains(&"channel-4"));
    }

    #[test]
    fn routing_destination_is_active_by_default() {
        let dest = RoutingDestination::new(test_destination());
        assert!(dest.is_active);
    }

    #[test]
    fn routing_destination_can_be_deactivated() {
        let mut dest = RoutingDestination::new(test_destination());
        dest.deactivate();
        assert!(!dest.is_active);
    }

    #[test]
    fn channel_entry_has_active_returns_true_when_active_destinations_exist() {
        let dest = RoutingDestination::new(test_destination());
        let entry = ChannelEntry::new(test_channel_id(), dest);
        assert!(entry.has_active());
    }

    #[test]
    fn channel_entry_has_active_returns_false_when_all_deactivated() {
        let dest = RoutingDestination::new(test_destination());
        let mut entry = ChannelEntry::new(test_channel_id(), dest);
        for d in &mut entry.destinations {
            d.deactivate();
        }
        assert!(!entry.has_active());
    }

    #[test]
    fn select_active_destinations_filters_inactive() {
        let dest1 = RoutingDestination::new(test_destination());
        let mut dest2 = RoutingDestination::new(test_destination());
        dest2.deactivate();

        let mut entry = ChannelEntry::new(test_channel_id(), dest1);
        entry.add_destination(dest2, 16).unwrap();

        let active = calc::select_active_destinations(&entry);
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn should_broadcast_returns_false_for_single_destination() {
        let dest = RoutingDestination::new(test_destination());
        let entry = ChannelEntry::new(test_channel_id(), dest);
        let config = RouterConfig::default();

        assert!(!calc::should_broadcast(&entry, &config));
    }

    #[test]
    fn should_broadcast_returns_true_for_multiple_destinations() {
        let dest1 = RoutingDestination::new(test_destination());
        let dest2 = RoutingDestination::new(test_destination());

        let mut entry = ChannelEntry::new(test_channel_id(), dest1);
        entry.add_destination(dest2, 16).unwrap();

        let config = RouterConfig::default();
        assert!(calc::should_broadcast(&entry, &config));
    }

    #[test]
    fn typed_message_new_creates_with_default_metadata() {
        let msg = TypedMessage::new(42i32);
        assert_eq!(*msg.payload(), 42);
        assert_eq!(msg.metadata().attempt, 0);
    }

    #[test]
    fn message_metadata_increment_attempt() {
        let metadata = MessageMetadata::default();
        let incremented = metadata.with_incremented_attempt();

        assert_eq!(incremented.attempt, 1);
        assert_eq!(incremented.message_id, metadata.message_id);
    }

    #[test]
    fn dead_letter_message_type_name_captured() {
        let msg = DeadLetterMessage {
            payload: vec![],
            type_name: std::any::type_name::<i32>().to_string(),
        };
        assert!(msg.type_name().contains("i32"));
    }

    #[tokio::test]
    async fn duplicate_message_is_rejected() {
        let config = RouterConfig::default();
        let mut router = MessageRouter::new(config);
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .unwrap();

        let msg = TypedMessage::new("test-payload".to_string());
        let result1 = router.route(&channel_id, msg.clone()).await;
        assert!(result1.is_ok());

        let result2 = router.route(&channel_id, msg).await;
        assert!(result2.is_err());
        assert!(matches!(result2.unwrap_err(), RouteError::DuplicateMessage(_)));
    }

    #[tokio::test]
    async fn different_messages_are_not_duplicates() {
        let config = RouterConfig::default();
        let mut router = MessageRouter::new(config);
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .unwrap();

        let msg1 = TypedMessage::new("payload-1".to_string());
        let msg2 = TypedMessage::new("payload-2".to_string());

        let result1 = router.route(&channel_id, msg1).await;
        assert!(result1.is_ok());

        let result2 = router.route(&channel_id, msg2).await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn deduplication_cache_tracks_seen_messages() {
        let config = RouterConfig::default();
        let mut router = MessageRouter::new(config);
        let channel_id = test_channel_id();
        let destination = test_destination();

        router
            .register_channel(channel_id.clone(), destination)
            .unwrap();

        assert_eq!(router.deduplication_cache_size(), 0);

        let msg = TypedMessage::new("test".to_string());
        router.route(&channel_id, msg).await.unwrap();

        assert_eq!(router.deduplication_cache_size(), 1);
    }

    #[test]
    fn deduplication_cache_size_accessor() {
        let config = RouterConfig::default();
        let mut router = MessageRouter::with_default_config();

        assert_eq!(router.deduplication_cache_size(), 0);

        router.clear_deduplication_cache();
        assert_eq!(router.deduplication_cache_size(), 0);
    }
}
