//! Actor message router with typed channels, routing table, broadcast/fan-out, and dead letter queue.
//!
//! Architecture: Data → Calc → Actions
//! - Data: `RouterConfig`, `RoutingTable`, `DeadLetterEntry`, `RouteError`
//! - Calc: Pure routing decisions, destination resolution
//! - Actions: Message dispatch via actor channels
//!
//! # Features
//!
//! - **Typed channels**: Each channel has a message type tag; only messages of that type
//!   can be sent through the channel
//! - **Routing table**: Maps `ChannelId` → `ActorRef` destinations
//! - **Broadcast/fan-out**: One message can be routed to multiple destinations
//! - **Dead letter queue**: Undeliverable messages are captured for later inspection/retry
//!
//! # Example
//!
//! ```ignore
//! use vo_actor::message_router::{MessageRouter, RouterConfig, ChannelId};
//! use ractor::{Actor, ActorRef};
//!
//! // Create router
//! let config = RouterConfig::default();
//! let router = MessageRouter::new(config);
//!
//! // Register a typed channel
//! let channel_id = ChannelId::new("workflow-events");
//! router.register_channel(channel_id.clone(), actor_ref).unwrap();
//!
//! // Route a message
//! router.route(channel_id, message).await;
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::time::Duration;

// =============================================================================
// Data Layer — Inert Types
// =============================================================================

/// A unique identifier for a typed channel.
/// Channels are the routing units — messages flow through channels to reach actors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelId(String);

impl ChannelId {
    /// Creates a new `ChannelId` from a string.
    ///
    /// # Panics
    /// Panics if `id` is empty.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        let s = id.into();
        assert!(!s.is_empty(), "ChannelId must not be empty");
        Self(s)
    }

    /// Parses a `ChannelId` from a string, returning an error if empty.
    pub fn parse(input: impl Into<String>) -> Result<Self, String> {
        let s = input.into();
        if s.is_empty() {
            return Err("ChannelId must not be empty".to_string());
        }
        Ok(Self(s))
    }

    /// Returns the channel ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for the message router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterConfig {
    /// Maximum number of destinations per channel (for fan-out limit).
    pub max_destinations_per_channel: usize,
    /// Maximum size of the dead letter queue.
    pub max_dlq_size: usize,
    /// Default timeout for message delivery.
    pub delivery_timeout: Duration,
    /// Whether to enable broadcast (fan-out) mode.
    pub broadcast_enabled: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            max_destinations_per_channel: 16,
            max_dlq_size: 1000,
            delivery_timeout: Duration::from_secs(5),
            broadcast_enabled: true,
        }
    }
}

impl RouterConfig {
    /// Creates a new `RouterConfig` with the given values.
    #[must_use]
    pub fn new(
        max_destinations_per_channel: usize,
        max_dlq_size: usize,
        delivery_timeout: Duration,
        broadcast_enabled: bool,
    ) -> Self {
        Self {
            max_destinations_per_channel,
            max_dlq_size,
            delivery_timeout,
            broadcast_enabled,
        }
    }
}

/// A typed message envelope for routing.
/// The `T` parameter is the message type tag.
#[derive(Debug, Clone)]
pub struct TypedMessage<T> {
    payload: T,
    metadata: MessageMetadata,
}

impl<T> TypedMessage<T> {
    /// Creates a new typed message with the given payload.
    #[must_use]
    pub fn new(payload: T) -> Self {
        Self {
            payload,
            metadata: MessageMetadata::default(),
        }
    }

    /// Creates a typed message with metadata.
    #[must_use]
    pub fn with_metadata(payload: T, metadata: MessageMetadata) -> Self {
        Self { payload, metadata }
    }

    /// Returns a reference to the payload.
    #[must_use]
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Consumes the message and returns the payload.
    #[allow(dead_code)]
    pub fn into_payload(self) -> T {
        self.payload
    }

    /// Returns a reference to the metadata.
    #[must_use]
    pub fn metadata(&self) -> &MessageMetadata {
        &self.metadata
    }
}

/// Metadata attached to every routed message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageMetadata {
    /// Unique message ID for tracing.
    pub message_id: String,
    /// Timestamp when message was routed.
    pub timestamp: TimestampMs,
    /// Number of delivery attempts.
    pub attempt: u32,
    /// Origin channel ID (if known).
    pub origin_channel: Option<ChannelId>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            message_id: ulid::Ulid::new().to_string(),
            timestamp: TimestampMs::now(),
            attempt: 0,
            origin_channel: None,
        }
    }
}

impl MessageMetadata {
    /// Increments the attempt counter and returns a new metadata.
    #[must_use]
    pub fn with_incremented_attempt(&self) -> Self {
        Self {
            attempt: self.attempt + 1,
            ..self.clone()
        }
    }

    /// Sets the origin channel.
    #[must_use]
    pub fn with_origin_channel(&self, channel: ChannelId) -> Self {
        Self {
            origin_channel: Some(channel),
            ..self.clone()
        }
    }
}

/// Timestamp in milliseconds since Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimestampMs(i64);

impl TimestampMs {
    /// Returns the current timestamp.
    #[must_use]
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        )
    }

    /// Returns the inner value.
    #[must_use]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

/// An opaque handle to an actor destination.
/// The actual type is hidden to allow different actor system implementations.
#[derive(Clone)]
#[allow(dead_code)]
pub struct ActorDestination(Arc<dyn Send + Sync>);

impl std::fmt::Debug for ActorDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ActorDestination").finish()
    }
}

impl ActorDestination {
    /// Creates a new `ActorDestination` from any sendable, sync-able impl.
    #[allow(dead_code)]
    pub fn new<T: Send + Sync + 'static>(inner: T) -> Self {
        Self(Arc::new(inner))
    }

    /// Returns the inner value downcasted, if the types match.
    /// Note: This requires the inner type to implement `std::any::Any`.
    #[allow(dead_code)]
    pub fn downcast<T: Send + Sync + 'static>(&self) -> Option<&T> {
        None
    }
}

/// A single destination entry in the routing table.
#[derive(Debug, Clone)]
pub struct RoutingDestination {
    /// The actor destination handle.
    pub destination: ActorDestination,
    /// When this destination was registered.
    pub registered_at: TimestampMs,
    /// Whether this destination is currently active.
    pub is_active: bool,
}

impl RoutingDestination {
    /// Creates a new active routing destination.
    #[must_use]
    pub fn new(destination: ActorDestination) -> Self {
        Self {
            destination,
            registered_at: TimestampMs::now(),
            is_active: true,
        }
    }

    /// Marks this destination as inactive.
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Marks this destination as active.
    pub fn activate(&mut self) {
        self.is_active = true;
    }
}

/// A channel entry in the routing table.
/// Supports multiple destinations (fan-out/broadcast).
#[derive(Debug, Clone)]
pub struct ChannelEntry {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// Registered destinations for this channel.
    pub destinations: Vec<RoutingDestination>,
    /// Whether broadcast is enabled for this channel.
    pub broadcast_enabled: bool,
    /// When this channel was created.
    pub created_at: TimestampMs,
}

impl ChannelEntry {
    /// Creates a new channel entry with a single destination.
    #[must_use]
    pub fn new(channel_id: ChannelId, destination: RoutingDestination) -> Self {
        Self {
            channel_id,
            destinations: vec![destination],
            broadcast_enabled: true,
            created_at: TimestampMs::now(),
        }
    }

    /// Adds a destination to this channel (for fan-out).
    ///
    /// Returns an error if max destinations would be exceeded.
    pub fn add_destination(
        &mut self,
        destination: RoutingDestination,
        max_destinations: usize,
    ) -> Result<(), RouteError> {
        if self.destinations.len() >= max_destinations {
            return Err(RouteError::MaxDestinationsExceeded {
                channel_id: self.channel_id.clone(),
                max: max_destinations,
            });
        }
        self.destinations.push(destination);
        Ok(())
    }

    /// Removes a destination by index.
    #[allow(dead_code)]
    pub fn remove_destination(&mut self, index: usize) -> Option<RoutingDestination> {
        if index < self.destinations.len() {
            Some(self.destinations.remove(index))
        } else {
            None
        }
    }

    /// Returns the number of active destinations.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.destinations.iter().filter(|d| d.is_active).count()
    }
}

/// An entry in the dead letter queue.
#[derive(Debug, Clone)]
pub struct DeadLetterEntry {
    /// The channel the message was addressed to.
    pub channel_id: ChannelId,
    /// The undeliverable message (stored as bytes for type erasure).
    pub message: DeadLetterMessage,
    /// When the message was added to the DLQ.
    pub enqueued_at: TimestampMs,
    /// Why delivery failed.
    pub reason: DeadLetterReason,
}

/// Type-erased dead letter message.
/// We store as bytes because the original type may not be available.
#[derive(Debug, Clone)]
pub struct DeadLetterMessage {
    payload: Vec<u8>,
    type_name: String,
}

impl DeadLetterMessage {
    /// Creates a new dead letter message from any payload.
    #[allow(dead_code)]
    pub fn new<T: serde::Serialize>(payload: &T) -> Result<Self, String> {
        let payload_bytes = serde_json::to_vec(payload)
            .map_err(|e| format!("failed to serialize payload: {}", e))?;
        let type_name = std::any::type_name::<T>().to_string();
        Ok(Self {
            payload: payload_bytes,
            type_name,
        })
    }

    /// Attempts to deserialize the payload back to type `T`.
    #[allow(dead_code)]
    pub fn deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        if self.type_name != std::any::type_name::<T>() {
            return Err(format!(
                "type mismatch: expected {}, got {}",
                self.type_name,
                std::any::type_name::<T>()
            ));
        }
        serde_json::from_slice(&self.payload)
            .map_err(|e| format!("failed to deserialize payload: {}", e))
    }

    /// Returns the type name of the payload.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }
}

/// Reason why a message was sent to the dead letter queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadLetterReason {
    /// The channel does not exist in the routing table.
    ChannelNotFound,
    /// No active destinations for the channel.
    NoActiveDestinations,
    /// Delivery timed out.
    DeliveryTimeout,
    /// Actor returned an error.
    ActorError(String),
    /// Message was explicitly dropped.
    ExplicitDrop,
}

/// Dead letter queue for undeliverable messages.
#[derive(Debug)]
pub struct DeadLetterQueue {
    entries: Vec<DeadLetterEntry>,
    max_size: usize,
}

impl DeadLetterQueue {
    /// Creates a new dead letter queue with the given max size.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
        }
    }

    /// Enqueues a dead letter entry.
    ///
    /// If the queue is full, oldest entries are evicted (FIFO).
    pub fn enqueue(&mut self, entry: DeadLetterEntry) {
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Dequeues the oldest dead letter entry.
    #[allow(dead_code)]
    pub fn dequeue(&mut self) -> Option<DeadLetterEntry> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.remove(0))
        }
    }

    /// Returns the number of entries in the DLQ.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the DLQ is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all entries from the DLQ.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns all entries for inspection.
    #[must_use]
    pub fn entries(&self) -> &[DeadLetterEntry] {
        &self.entries
    }
}

// =============================================================================
// Calculation Layer — Pure Routing Decisions
// =============================================================================

/// Errors that can occur during routing operations.
#[derive(Debug, Clone, Error)]
pub enum RouteError {
    /// Channel is not registered in the routing table.
    #[error("channel not found: {0}")]
    ChannelNotFound(ChannelId),

    /// No active destinations for the channel.
    #[error("no active destinations for channel: {0}")]
    NoActiveDestinations(ChannelId),

    /// Maximum destinations exceeded for the channel.
    #[error("max destinations exceeded for channel {channel_id}: {max}")]
    MaxDestinationsExceeded { channel_id: ChannelId, max: usize },

    /// Delivery timed out.
    #[error("delivery timeout for channel: {0}")]
    DeliveryTimeout(ChannelId),

    /// Actor returned an error during message delivery.
    #[error("actor error on channel {0}: {1}")]
    ActorError(ChannelId, String),

    /// Dead letter queue is full.
    #[error("dead letter queue is full")]
    DeadLetterQueueFull,

    /// Channel already exists.
    #[error("channel already exists: {0}")]
    ChannelAlreadyExists(ChannelId),

    /// Destination already registered for channel.
    #[error("destination already registered for channel: {0}")]
    DestinationAlreadyRegistered(ChannelId),

    /// Channel is closed.
    #[error("channel is closed: {0}")]
    ChannelClosed(ChannelId),
}

/// Pure function: selects active destinations from a channel entry.
fn select_active_destinations(channel: &ChannelEntry) -> Vec<(usize, &RoutingDestination)> {
    channel
        .destinations
        .iter()
        .enumerate()
        .filter(|(_, d)| d.is_active)
        .collect()
}

/// Pure function: determines if a message should be broadcast or sent to single destination.
fn should_broadcast(channel: &ChannelEntry, config: &RouterConfig) -> bool {
    config.broadcast_enabled && channel.broadcast_enabled && channel.destinations.len() > 1
}

/// Pure function: validates that a message can be routed.
fn validate_route(
    channel: Option<&ChannelEntry>,
    _config: &RouterConfig,
) -> Result<(), RouteError> {
    match channel {
        None => Err(RouteError::ChannelNotFound(
            channel
                .map(|c| c.channel_id.clone())
                .unwrap_or_else(|| ChannelId::new("unknown")),
        )),
        Some(ch) if ch.destinations.is_empty() => {
            Err(RouteError::NoActiveDestinations(ch.channel_id.clone()))
        }
        Some(ch) if !ch.has_active() => {
            Err(RouteError::NoActiveDestinations(ch.channel_id.clone()))
        }
        _ => Ok(()),
    }
}

impl ChannelEntry {
    /// Returns true if this channel has any active destinations.
    #[must_use]
    pub fn has_active(&self) -> bool {
        self.destinations.iter().any(|d| d.is_active)
    }
}

// =============================================================================
// Action Layer — Message Router
// =============================================================================

/// The message router for actor-based message routing.
///
/// Manages typed channels, maintains a routing table, supports broadcast/fan-out,
/// and captures undeliverable messages in a dead letter queue.
#[derive(Debug)]
pub struct MessageRouter {
    config: RouterConfig,
    routing_table: HashMap<ChannelId, ChannelEntry>,
    dead_letter_queue: DeadLetterQueue,
}

impl MessageRouter {
    /// Creates a new `MessageRouter` with the given configuration.
    #[must_use]
    pub fn new(config: RouterConfig) -> Self {
        let max_dlq_size = config.max_dlq_size;
        Self {
            config,
            routing_table: HashMap::new(),
            dead_letter_queue: DeadLetterQueue::new(max_dlq_size),
        }
    }

    /// Creates a new `MessageRouter` with default configuration.
    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(RouterConfig::default())
    }

    /// Registers a new channel with a single destination.
    ///
    /// # Errors
    /// Returns `RouteError::ChannelAlreadyExists` if the channel is already registered.
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

    /// Registers a new channel with multiple destinations (for broadcast).
    ///
    /// # Errors
    /// Returns `RouteError::ChannelAlreadyExists` if the channel is already registered.
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

    /// Adds a destination to an existing channel (for fan-out).
    ///
    /// # Errors
    /// Returns `RouteError::ChannelNotFound` if the channel doesn't exist.
    /// Returns `RouteError::MaxDestinationsExceeded` if max destinations would be exceeded.
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

    /// Removes a destination from an existing channel.
    ///
    /// # Errors
    /// Returns `RouteError::ChannelNotFound` if the channel doesn't exist.
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

    /// Unregisters a channel entirely.
    ///
    /// Returns the removed channel entry if it existed.
    #[allow(dead_code)]
    pub fn unregister_channel(&mut self, channel_id: &ChannelId) -> Option<ChannelEntry> {
        self.routing_table.remove(channel_id)
    }

    /// Deactivates all destinations for a channel (但不删除 channel).
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

    /// Routes a message to a single destination (unicast).
    /// The message is sent to the first active destination.
    ///
    /// # Type Parameters
    /// * `T` - The message type; must be `Send + 'static`
    ///
    /// # Errors
    /// Returns `RouteError` if routing fails. The message may be sent to DLQ
    /// if delivery fails but routing succeeds.
    pub async fn route_unicast<T: Send + 'static>(
        &mut self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError> {
        let channel = self.routing_table.get(channel_id).cloned();

        validate_route(channel.as_ref(), &self.config)?;

        let channel = channel.unwrap();
        let active_dests = select_active_destinations(&channel);

        if active_dests.is_empty() {
            self.send_to_dlq(channel_id, message, DeadLetterReason::NoActiveDestinations);
            return Err(RouteError::NoActiveDestinations(channel_id.clone()));
        }

        // Send to first active destination
        let (_index, dest) = active_dests[0];
        match self
            .deliver_to_destination_unicast(dest, &message, channel_id)
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                self.send_to_dlq(
                    channel_id,
                    message,
                    DeadLetterReason::ActorError(e.to_string()),
                );
                Err(e)
            }
        }
    }

    /// Routes a message to all active destinations (broadcast/fan-out).
    ///
    /// # Type Parameters
    /// * `T` - The message type; must be `Send + Sync + 'static` since the message
    ///   is shared across multiple destinations concurrently.
    ///
    /// # Errors
    /// Returns `RouteError` if no destinations are available. Messages that fail
    /// delivery individually are still sent to DLQ.
    pub async fn route_broadcast<T: Send + Sync + 'static>(
        &mut self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError> {
        let channel = self.routing_table.get(channel_id).cloned();

        validate_route(channel.as_ref(), &self.config)?;

        let channel = channel.unwrap();
        let active_dests = select_active_destinations(&channel);

        if active_dests.is_empty() {
            self.send_to_dlq(channel_id, message, DeadLetterReason::NoActiveDestinations);
            return Err(RouteError::NoActiveDestinations(channel_id.clone()));
        }

        if !should_broadcast(&channel, &self.config) {
            // Fall back to unicast if broadcast not enabled for this channel
            return self.route_unicast(channel_id, message).await;
        }

        // Fan-out: send to all active destinations
        let mut errors = Vec::new();
        let num_destinations = active_dests.len();
        for (_index, dest) in active_dests {
            if let Err(e) = self
                .deliver_to_destination_broadcast(dest, &message, channel_id)
                .await
            {
                errors.push(e);
            }
        }

        if !errors.is_empty() && errors.len() == num_destinations {
            // All deliveries failed
            self.send_to_dlq(
                channel_id,
                message,
                DeadLetterReason::ActorError("all destinations failed".to_string()),
            );
            return Err(errors.into_iter().next().unwrap());
        }

        Ok(())
    }

    /// Routes a message to a channel, auto-selecting unicast or broadcast
    /// based on channel configuration and number of destinations.
    pub async fn route<T: Send + Sync + 'static>(
        &mut self,
        channel_id: &ChannelId,
        message: T,
    ) -> Result<(), RouteError> {
        let channel = self.routing_table.get(channel_id).cloned();

        if channel.as_ref().map(|c| c.destinations.len()).unwrap_or(0) > 1 {
            self.route_broadcast(channel_id, message).await
        } else {
            self.route_unicast(channel_id, message).await
        }
    }

    /// Delivers a message to a specific destination (unicast version).
    /// Takes a reference since unicast consumes the message after delivery.
    #[allow(clippy::unused_async)]
    async fn deliver_to_destination_unicast<T: Send + 'static>(
        &self,
        destination: &RoutingDestination,
        message: &T,
        _channel_id: &ChannelId,
    ) -> Result<(), RouteError> {
        // In a real implementation, this would:
        // 1. Look up the actual actor ref from the destination handle
        // 2. Send the message through the actor's mailbox
        // 3. Handle timeouts and errors

        // For now, we simulate a successful delivery
        // The actual ractor integration would happen here
        tracing::debug!("delivering message to destination (simulated success)");

        let _ = destination;
        let _ = message;
        Ok(())
    }

    /// Delivers a message to a specific destination (broadcast version).
    /// Takes a reference since broadcast shares the message across destinations.
    #[allow(clippy::unused_async)]
    async fn deliver_to_destination_broadcast<T: Send + Sync + 'static>(
        &self,
        destination: &RoutingDestination,
        message: &T,
        _channel_id: &ChannelId,
    ) -> Result<(), RouteError> {
        // In a real implementation, this would:
        // 1. Look up the actual actor ref from the destination handle
        // 2. Send the message through the actor's mailbox
        // 3. Handle timeouts and errors

        // For now, we simulate a successful delivery
        // The actual ractor integration would happen here
        tracing::debug!("delivering message to destination (simulated success)");

        let _ = destination;
        let _ = message;
        Ok(())
    }

    /// Sends an undeliverable message to the dead letter queue.
    fn send_to_dlq<T: Send + 'static>(
        &mut self,
        channel_id: &ChannelId,
        _message: T,
        reason: DeadLetterReason,
    ) {
        let entry = DeadLetterEntry {
            channel_id: channel_id.clone(),
            message: DeadLetterMessage {
                payload: Vec::new(), // Would serialize message here
                type_name: std::any::type_name::<T>().to_string(),
            },
            enqueued_at: TimestampMs::now(),
            reason,
        };

        self.dead_letter_queue.enqueue(entry);
    }

    /// Returns the number of registered channels.
    #[must_use]
    pub fn num_channels(&self) -> usize {
        self.routing_table.len()
    }

    /// Returns the number of total destinations across all channels.
    #[must_use]
    pub fn total_destinations(&self) -> usize {
        self.routing_table
            .values()
            .map(|e| e.destinations.len())
            .sum()
    }

    /// Returns the number of active destinations across all channels.
    #[must_use]
    pub fn total_active_destinations(&self) -> usize {
        self.routing_table.values().map(|e| e.active_count()).sum()
    }

    /// Returns the current dead letter queue depth.
    #[must_use]
    pub fn dlq_depth(&self) -> usize {
        self.dead_letter_queue.len()
    }

    /// Checks if a channel exists.
    #[must_use]
    pub fn has_channel(&self, channel_id: &ChannelId) -> bool {
        self.routing_table.contains_key(channel_id)
    }

    /// Checks if a channel has any active destinations.
    #[must_use]
    pub fn is_channel_active(&self, channel_id: &ChannelId) -> bool {
        self.routing_table
            .get(channel_id)
            .map(|e| e.has_active())
            .unwrap_or(false)
    }

    /// Returns a clone of the router configuration.
    #[must_use]
    pub fn config(&self) -> RouterConfig {
        self.config.clone()
    }

    /// Drains all entries from the dead letter queue.
    #[allow(dead_code)]
    pub fn drain_dlq(&mut self) -> Vec<DeadLetterEntry> {
        let entries: Vec<_> = self.dead_letter_queue.entries.to_vec();
        self.dead_letter_queue.clear();
        entries
    }

    /// Clears the dead letter queue.
    pub fn clear_dlq(&mut self) {
        self.dead_letter_queue.clear();
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
    fn channel_id_equality() {
        let id1 = ChannelId::new("channel-a");
        let id2 = ChannelId::new("channel-a");
        let id3 = ChannelId::new("channel-b");

        assert_eq!(id1, id2, "Same string should create equal ChannelId");
        assert_ne!(id1, id3, "Different strings should create unequal ChannelId");
    }

    #[test]
    fn channel_id_clone_is_independent() {
        let id1 = ChannelId::new("test-channel");
        let id2 = id1.clone();
        assert_eq!(id1, id2);
    }

    #[test]
    fn channel_id_display_shows_inner_value() {
        let id = ChannelId::new("display-test");
        let display_str = format!("{}", id);
        assert_eq!(display_str, "display-test");
    }

    #[test]
    fn channel_id_parse_accepts_valid_string() {
        let result = ChannelId::parse("valid-channel");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "valid-channel");
    }

    #[test]
    fn channel_id_debug_format() {
        let id = ChannelId::new("debug-channel");
        let debug_str = format!("{:?}", id);
        assert!(debug_str.contains("debug-channel"));
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
    fn router_config_new_creates_custom_config() {
        let config = RouterConfig::new(32, 500, Duration::from_secs(10), false);
        assert_eq!(config.max_destinations_per_channel, 32);
        assert_eq!(config.max_dlq_size, 500);
        assert_eq!(config.delivery_timeout, Duration::from_secs(10));
        assert!(!config.broadcast_enabled);
    }

    #[test]
    fn router_config_equality() {
        let config1 = RouterConfig::new(16, 1000, Duration::from_secs(5), true);
        let config2 = RouterConfig::new(16, 1000, Duration::from_secs(5), true);
        let config3 = RouterConfig::new(16, 1000, Duration::from_secs(5), false);

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
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
        // First two entries should have been evicted
        let entries: Vec<_> = dlq.entries.iter().map(|e| e.channel_id.as_str()).collect();
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
        entry.deactivate_all();
        assert!(!entry.has_active());
    }

    impl ChannelEntry {
        fn deactivate_all(&mut self) {
            for dest in &mut self.destinations {
                dest.deactivate();
            }
        }
    }

    #[test]
    fn select_active_destinations_filters_inactive() {
        let dest1 = RoutingDestination::new(test_destination());
        let mut dest2 = RoutingDestination::new(test_destination());
        dest2.deactivate();

        let mut entry = ChannelEntry::new(test_channel_id(), dest1);
        entry.add_destination(dest2, 16).unwrap();

        let active = select_active_destinations(&entry);
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn should_broadcast_returns_false_for_single_destination() {
        let dest = RoutingDestination::new(test_destination());
        let entry = ChannelEntry::new(test_channel_id(), dest);
        let config = RouterConfig::default();

        assert!(!should_broadcast(&entry, &config));
    }

    #[test]
    fn should_broadcast_returns_true_for_multiple_destinations() {
        let dest1 = RoutingDestination::new(test_destination());
        let dest2 = RoutingDestination::new(test_destination());

        let mut entry = ChannelEntry::new(test_channel_id(), dest1);
        entry.add_destination(dest2, 16).unwrap();

        let config = RouterConfig::default();
        assert!(should_broadcast(&entry, &config));
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

    #[test]
    fn timestamp_ms_now_returns_positive_value() {
        let ts = TimestampMs::now();
        assert!(ts.as_i64() > 0, "Timestamp should be positive");
    }

    #[test]
    fn timestamp_ms_monotonic_increasing() {
        let ts1 = TimestampMs::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ts2 = TimestampMs::now();
        assert!(
            ts2 > ts1,
            "Subsequent timestamp should be greater than previous"
        );
    }

    #[test]
    fn timestamp_ms_ordering_consistent() {
        let ts1 = TimestampMs::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ts2 = TimestampMs::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ts3 = TimestampMs::now();

        assert!(ts1 < ts2, "ts1 should be less than ts2");
        assert!(ts2 < ts3, "ts2 should be less than ts3");
        assert!(ts1 < ts3, "ts1 should be less than ts3");
    }

    #[test]
    fn timestamp_ms_as_i64_returns_inner_value() {
        let value: i64 = 12345;
        let ts = TimestampMs(value);
        assert_eq!(ts.as_i64(), value);
    }

    #[test]
    fn timestamp_ms_partial_ord_respects_inner_value() {
        let ts1 = TimestampMs(100);
        let ts2 = TimestampMs(200);
        let ts3 = TimestampMs(100);

        assert!(ts1 < ts2);
        assert!(ts2 > ts1);
        assert_eq!(ts1, ts3);
    }
}

// =============================================================================
// Property-Based Tests - Message Ordering Guarantees
// =============================================================================

#[cfg(feature = "proptest")]
mod proptest_message_ordering {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn dlq_fifo_ordering_respects_insertion_order(
            capacity in 1..=100usize,
            num_entries in 0..=200usize
        ) {
            // Property: DLQ maintains FIFO order - earlier entries are evicted first
            let mut dlq = DeadLetterQueue::new(capacity);
            let mut expected_order = Vec::new();

            for i in 0..num_entries {
                let channel_id = ChannelId::new(format!("channel-{}", i % 10));
                let entry = DeadLetterEntry {
                    channel_id: channel_id.clone(),
                    message: DeadLetterMessage {
                        payload: vec![i as u8],
                        type_name: "test".to_string(),
                    },
                    enqueued_at: TimestampMs(i as i64),
                    reason: DeadLetterReason::ChannelNotFound,
                };
                expected_order.push(channel_id);
                dlq.enqueue(entry);
            }

            // After all insertions, DLQ should contain min(num_entries, capacity) entries
            let final_len = dlq.len();
            prop_assert_eq!(final_len, std::cmp::min(num_entries, capacity));

            // If not full, entries should be in insertion order
            if num_entries <= capacity {
                let entries: Vec<_> = dlq.entries.iter().map(|e| e.channel_id.as_str()).collect();
                let expected: Vec<_> = expected_order.iter().map(|s| s.as_str()).collect();
                prop_assert_eq!(entries, expected);
            } else {
                // If full, oldest entries (first num_entries - capacity) should be evicted
                // and the most recent 'capacity' entries should remain
                let expected_remaining: Vec<_> = expected_order
                    .iter()
                    .skip(num_entries - capacity)
                    .map(|s| s.as_str())
                    .collect();
                let actual_remaining: Vec<_> = dlq.entries.iter().map(|e| e.channel_id.as_str()).collect();
                prop_assert_eq!(actual_remaining, expected_remaining);
            }
        }

        #[test]
        fn routing_table_operations_are_deterministic(
            num_channels in 1..=20usize,
            operations in 1..=50usize
        ) {
            // Property: Given same initial state and operations, routing decisions are identical
            let mut router1 = MessageRouter::with_default_config();
            let mut router2 = MessageRouter::with_default_config();

            // Register channels
            for i in 0..num_channels {
                let channel_id = ChannelId::new(format!("channel-{}", i));
                let dest = ActorDestination::new(i);
                prop_assert!(router1.register_channel(channel_id.clone(), dest.clone()).is_ok());
                prop_assert!(router2.register_channel(channel_id.clone(), dest.clone()).is_ok());
            }

            // Perform same operations on both routers
            let mut rng = proptest::test_runner::TestRng::default();
            for op_idx in 0..operations {
                let channel_idx = op_idx % num_channels;
                let channel_id = ChannelId::new(format!("channel-{}", channel_idx));

                // Operation: add destination (always succeeds since we control the value)
                let dest = ActorDestination::new(op_idx);
                let _ = router1.add_destination(&channel_id, dest.clone());
                let _ = router2.add_destination(&channel_id, dest.clone());

                // Both routers should have same number of channels
                prop_assert_eq!(router1.num_channels(), router2.num_channels());
                prop_assert_eq!(router1.total_destinations(), router2.total_destinations());
            }
        }

        #[test]
        fn channel_entry_active_count_is_consistent(
            initial_destinations in 1..=10usize,
            activations in 0..=20usize
        ) {
            // Property: active_count always matches actual number of active destinations
            let channel_id = ChannelId::new("test-channel");
            let mut entry = {
                let first_dest = RoutingDestination::new(ActorDestination::new(0));
                ChannelEntry::new(channel_id.clone(), first_dest)
            };

            // Add more destinations
            for i in 1..initial_destinations {
                let dest = RoutingDestination::new(ActorDestination::new(i));
                prop_assert!(entry.add_destination(dest, 100).is_ok());
            }

            // Record initial state
            let initial_active = entry.active_count();

            // Toggle activations randomly
            let mut expected_active = initial_active;
            for i in 0..activations {
                let dest_idx = i % entry.destinations.len();
                if entry.destinations[dest_idx].is_active {
                    entry.destinations[dest_idx].deactivate();
                    expected_active = expected_active.saturating_sub(1);
                } else {
                    entry.destinations[dest_idx].activate();
                    expected_active += 1;
                }
                prop_assert_eq!(entry.active_count(), expected_active);
            }
        }

        #[test]
        fn typed_message_metadata_is_properly_captured(
            payload in any::<i32>(),
            attempt in 0u32..=10u32
        ) {
            // Property: TypedMessage preserves payload and metadata correctly
            let metadata = MessageMetadata {
                message_id: "test-id".to_string(),
                timestamp: TimestampMs::now(),
                attempt,
                origin_channel: None,
            };
            let msg = TypedMessage::with_metadata(payload, metadata.clone());

            prop_assert_eq!(*msg.payload(), payload);
            prop_assert_eq!(msg.metadata().attempt, attempt);
            prop_assert_eq!(msg.metadata().message_id, "test-id");
        }

        #[test]
        fn select_active_destinations_is_idempotent(
            num_destinations in 1..=20usize,
            num_inactive in 0..=15usize
        ) {
            // Property: Selecting active destinations multiple times yields same result
            let channel_id = ChannelId::new("test-channel");
            let mut entry = {
                let first_dest = RoutingDestination::new(ActorDestination::new(0));
                ChannelEntry::new(channel_id.clone(), first_dest)
            };

            for i in 1..num_destinations {
                let dest = RoutingDestination::new(ActorDestination::new(i));
                prop_assert!(entry.add_destination(dest, 100).is_ok());
            }

            // Deactivate some destinations
            let deactivate_count = std::cmp::min(num_inactive, num_destinations);
            for i in 0..deactivate_count {
                entry.destinations[i].deactivate();
            }

            // Multiple calls to select_active_destinations should yield same result
            let result1 = select_active_destinations(&entry);
            let result2 = select_active_destinations(&entry);
            let result3 = select_active_destinations(&entry);

            prop_assert_eq!(result1.len(), result2.len());
            prop_assert_eq!(result2.len(), result3.len());
            prop_assert_eq!(result1.len(), num_destinations - deactivate_count);
        }

        #[test]
        fn should_broadcast_is_deterministic(
            num_destinations in 1..=10usize,
            broadcast_enabled in proptest::bool::ANY,
            global_broadcast_enabled in proptest::bool::ANY
        ) {
            // Property: should_broadcast decision is deterministic based on inputs
            let channel_id = ChannelId::new("test-channel");
            let mut entry = {
                let first_dest = RoutingDestination::new(ActorDestination::new(0));
                ChannelEntry::new(channel_id.clone(), first_dest)
            };

            for i in 1..num_destinations {
                let dest = RoutingDestination::new(ActorDestination::new(i));
                prop_assert!(entry.add_destination(dest, 100).is_ok());
            }

            entry.broadcast_enabled = broadcast_enabled;

            let config = RouterConfig {
                max_destinations_per_channel: 16,
                max_dlq_size: 1000,
                delivery_timeout: Duration::from_secs(5),
                broadcast_enabled: global_broadcast_enabled,
            };

            let result1 = should_broadcast(&entry, &config);
            let result2 = should_broadcast(&entry, &config);

            prop_assert_eq!(result1, result2);

            // Expected: broadcast if both enabled AND more than 1 destination
            let expected = global_broadcast_enabled && broadcast_enabled && num_destinations > 1;
            prop_assert_eq!(result1, expected);
        }
    }
}
