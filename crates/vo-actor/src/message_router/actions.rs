//! Action Layer — Message Router Operations.
//!
//! The `MessageRouter` struct and all its mutating/querying methods.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::calc::{select_active_destinations, should_broadcast, validate_route, RouteError};
use super::data::{
    ActorDestination, ChannelEntry, ChannelId, RouterConfig, RoutingDestination, TimestampMs,
    TypedMessage,
};
use super::dlq::{DeadLetterEntry, DeadLetterMessage, DeadLetterQueue, DeadLetterReason};

type DeduplicationCache = HashMap<String, Instant>;

#[derive(Debug)]
pub struct MessageRouter {
    config: RouterConfig,
    routing_table: HashMap<ChannelId, ChannelEntry>,
    dead_letter_queue: DeadLetterQueue,
    deduplication_cache: DeduplicationCache,
}

impl MessageRouter {
    #[must_use]
    pub fn new(config: RouterConfig) -> Self {
        let max_dlq_size = config.max_dlq_size;
        Self {
            config: config.clone(),
            routing_table: HashMap::new(),
            dead_letter_queue: DeadLetterQueue::new(max_dlq_size),
            deduplication_cache: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(RouterConfig::default())
    }

    pub fn register_channel(
        &mut self,
        channel_id: ChannelId,
        destination: ActorDestination,
    ) -> Result<(), RouteError> {
        if self.routing_table.contains_key(&channel_id) {
            return Err(RouteError::ChannelAlreadyExists(channel_id));
        }
        let entry = ChannelEntry::new(channel_id.clone(), RoutingDestination::new(destination));
        self.routing_table.insert(channel_id, entry);
        Ok(())
    }

    pub fn register_broadcast_channel(
        &mut self,
        channel_id: ChannelId,
        destinations: Vec<ActorDestination>,
    ) -> Result<(), RouteError> {
        if self.routing_table.contains_key(&channel_id) {
            return Err(RouteError::ChannelAlreadyExists(channel_id));
        }
        let routing_destinations: Vec<RoutingDestination> = destinations
            .into_iter()
            .map(RoutingDestination::new)
            .collect();
        let entry = ChannelEntry {
            channel_id: channel_id.clone(),
            destinations: routing_destinations,
            broadcast_enabled: true,
            created_at: TimestampMs::now(),
        };
        self.routing_table.insert(channel_id, entry);
        Ok(())
    }

    pub fn add_destination(
        &mut self,
        channel_id: &ChannelId,
        destination: ActorDestination,
    ) -> Result<(), RouteError> {
        let entry = self
            .routing_table
            .get_mut(channel_id)
            .ok_or_else(|| RouteError::ChannelNotFound(channel_id.clone()))?;
        entry.add_destination(
            RoutingDestination::new(destination),
            self.config.max_destinations_per_channel,
        )
    }

    #[allow(dead_code)]
    pub fn remove_destination(
        &mut self,
        channel_id: &ChannelId,
        index: usize,
    ) -> Result<(), RouteError> {
        let entry = self
            .routing_table
            .get_mut(channel_id)
            .ok_or_else(|| RouteError::ChannelNotFound(channel_id.clone()))?;
        entry.remove_destination(index);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn unregister_channel(&mut self, channel_id: &ChannelId) -> Option<ChannelEntry> {
        self.routing_table.remove(channel_id)
    }

    #[allow(dead_code)]
    pub fn deactivate_channel(&mut self, channel_id: &ChannelId) -> Result<(), RouteError> {
        let entry = self
            .routing_table
            .get_mut(channel_id)
            .ok_or_else(|| RouteError::ChannelNotFound(channel_id.clone()))?;
        for dest in &mut entry.destinations {
            dest.deactivate();
        }
        Ok(())
    }

    pub async fn route_unicast<T: Send + 'static>(
        &mut self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        if self.is_duplicate(&message) {
            return Err(RouteError::DuplicateMessage(message.metadata().message_id.clone()));
        }
        self.evict_expired_entries();
        self.deduplication_cache
            .insert(message.metadata().message_id.clone(), Instant::now());

        let channel = self.routing_table.get(channel_id).cloned();
        validate_route(channel.as_ref(), &self.config)?;
        let channel = channel.unwrap();
        let active_dests = select_active_destinations(&channel);
        if active_dests.is_empty() {
            self.send_to_dlq(channel_id, message.into_payload(), DeadLetterReason::NoActiveDestinations);
            return Err(RouteError::NoActiveDestinations(channel_id.clone()));
        }
        let (_index, dest) = active_dests[0];
        let payload = message.into_payload();
        match self
            .deliver_to_destination_unicast(dest, &payload, channel_id)
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                self.send_to_dlq(
                    channel_id,
                    payload,
                    DeadLetterReason::ActorError(e.to_string()),
                );
                Err(e)
            }
        }
    }

    pub async fn route_broadcast<T: Send + Sync + 'static>(
        &mut self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        if self.is_duplicate(&message) {
            return Err(RouteError::DuplicateMessage(message.metadata().message_id.clone()));
        }
        self.evict_expired_entries();
        self.deduplication_cache
            .insert(message.metadata().message_id.clone(), Instant::now());

        let channel = self.routing_table.get(channel_id).cloned();
        validate_route(channel.as_ref(), &self.config)?;
        let channel = channel.unwrap();
        let active_dests = select_active_destinations(&channel);
        if active_dests.is_empty() {
            self.send_to_dlq(channel_id, message.into_payload(), DeadLetterReason::NoActiveDestinations);
            return Err(RouteError::NoActiveDestinations(channel_id.clone()));
        }
        if !should_broadcast(&channel, &self.config) {
            let msg: TypedMessage<T> = message;
            return self.route_unicast(channel_id, msg).await;
        }
        let mut errors = Vec::new();
        let num_destinations = active_dests.len();
        let payload = message.into_payload();
        for (_index, dest) in active_dests {
            if let Err(e) = self
                .deliver_to_destination_broadcast(dest, &payload, channel_id)
                .await
            {
                errors.push(e);
            }
        }
        if !errors.is_empty() && errors.len() == num_destinations {
            self.send_to_dlq(
                channel_id,
                payload,
                DeadLetterReason::ActorError("all destinations failed".to_string()),
            );
            return Err(errors.into_iter().next().unwrap());
        }
        Ok(())
    }

    pub async fn route<T: Send + Sync + 'static>(
        &mut self,
        channel_id: &ChannelId,
        message: TypedMessage<T>,
    ) -> Result<(), RouteError> {
        let channel = self.routing_table.get(channel_id).cloned();
        if channel.as_ref().map(|c| c.destinations.len()).unwrap_or(0) > 1 {
            self.route_broadcast(channel_id, message).await
        } else {
            self.route_unicast(channel_id, message).await
        }
    }

    fn is_duplicate<T>(&self, message: &TypedMessage<T>) -> bool {
        if let Some(instant) = self.deduplication_cache.get(&message.metadata().message_id) {
            if instant.elapsed() < self.config.deduplication_ttl {
                return true;
            }
        }
        false
    }

    fn evict_expired_entries(&mut self) {
        if self.deduplication_cache.len() >= self.config.max_deduplication_entries {
            let ttl = self.config.deduplication_ttl;
            self.deduplication_cache.retain(|_id, instant| instant.elapsed() < ttl);
        }
        if self.deduplication_cache.len() >= self.config.max_deduplication_entries {
            let half = self.deduplication_cache.len() / 2;
            let mut keys_to_remove: Vec<_> = self.deduplication_cache.keys().cloned().collect();
            keys_to_remove.sort_by_key(|k| self.deduplication_cache.get(k));
            for key in keys_to_remove.into_iter().take(half) {
                self.deduplication_cache.remove(&key);
            }
        }
    }

    #[allow(clippy::unused_async)]
    async fn deliver_to_destination_unicast<T: Send + 'static>(
        &self,
        destination: &RoutingDestination,
        message: &T,
        _channel_id: &ChannelId,
    ) -> Result<(), RouteError> {
        tracing::debug!("delivering message to destination (simulated success)");
        let _ = destination;
        let _ = message;
        Ok(())
    }

    #[allow(clippy::unused_async)]
    async fn deliver_to_destination_broadcast<T: Send + Sync + 'static>(
        &self,
        destination: &RoutingDestination,
        message: &T,
        _channel_id: &ChannelId,
    ) -> Result<(), RouteError> {
        tracing::debug!("delivering message to destination (simulated success)");
        let _ = destination;
        let _ = message;
        Ok(())
    }

    fn send_to_dlq<T: Send + 'static>(
        &mut self,
        channel_id: &ChannelId,
        _message: T,
        reason: DeadLetterReason,
    ) {
        let entry = DeadLetterEntry {
            channel_id: channel_id.clone(),
            message: DeadLetterMessage {
                payload: Vec::new(),
                type_name: std::any::type_name::<T>().to_string(),
            },
            enqueued_at: TimestampMs::now(),
            reason,
        };
        self.dead_letter_queue.enqueue(entry);
    }

    #[must_use]
    pub fn num_channels(&self) -> usize {
        self.routing_table.len()
    }

    #[must_use]
    pub fn total_destinations(&self) -> usize {
        self.routing_table
            .values()
            .map(|e| e.destinations.len())
            .sum()
    }

    #[must_use]
    pub fn total_active_destinations(&self) -> usize {
        self.routing_table.values().map(|e| e.active_count()).sum()
    }

    #[must_use]
    pub fn dlq_depth(&self) -> usize {
        self.dead_letter_queue.len()
    }

    #[must_use]
    pub fn has_channel(&self, channel_id: &ChannelId) -> bool {
        self.routing_table.contains_key(channel_id)
    }

    #[must_use]
    pub fn is_channel_active(&self, channel_id: &ChannelId) -> bool {
        self.routing_table
            .get(channel_id)
            .map(|e| e.has_active())
            .unwrap_or(false)
    }

    #[must_use]
    pub fn config(&self) -> RouterConfig {
        self.config.clone()
    }

    #[allow(dead_code)]
    pub fn drain_dlq(&mut self) -> Vec<DeadLetterEntry> {
        let entries: Vec<_> = self.dead_letter_queue.entries().to_vec();
        self.dead_letter_queue.clear();
        entries
    }

    pub fn clear_dlq(&mut self) {
        self.dead_letter_queue.clear();
    }

    #[must_use]
    pub fn deduplication_cache_size(&self) -> usize {
        self.deduplication_cache.len()
    }

    pub fn clear_deduplication_cache(&mut self) {
        self.deduplication_cache.clear();
    }
}
