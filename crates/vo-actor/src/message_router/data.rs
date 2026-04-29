//! Data Layer — Inert Types for Message Router.
//!
//! All types here are pure data with no side effects.

use std::sync::Arc;
use tokio::time::Duration;

/// A unique identifier for a typed channel.
/// Channels are the routing units — messages flow through channels to reach actors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelId(String);

impl ChannelId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        let s = id.into();
        assert!(!s.is_empty(), "ChannelId must not be empty");
        Self(s)
    }

    pub fn parse(input: impl Into<String>) -> Result<Self, String> {
        let s = input.into();
        if s.is_empty() {
            return Err("ChannelId must not be empty".to_string());
        }
        Ok(Self(s))
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterConfig {
    pub max_destinations_per_channel: usize,
    pub max_dlq_size: usize,
    pub delivery_timeout: Duration,
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

#[derive(Debug, Clone)]
pub struct TypedMessage<T> {
    payload: T,
    metadata: MessageMetadata,
}

impl<T> TypedMessage<T> {
    #[must_use]
    pub fn new(payload: T) -> Self {
        Self {
            payload,
            metadata: MessageMetadata::default(),
        }
    }

    #[must_use]
    pub fn with_metadata(payload: T, metadata: MessageMetadata) -> Self {
        Self { payload, metadata }
    }

    #[must_use]
    pub fn payload(&self) -> &T {
        &self.payload
    }

    #[allow(dead_code)]
    pub fn into_payload(self) -> T {
        self.payload
    }

    #[must_use]
    pub fn metadata(&self) -> &MessageMetadata {
        &self.metadata
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageMetadata {
    pub message_id: String,
    pub timestamp: TimestampMs,
    pub attempt: u32,
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
    #[must_use]
    pub fn with_incremented_attempt(&self) -> Self {
        Self {
            attempt: self.attempt + 1,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn with_origin_channel(&self, channel: ChannelId) -> Self {
        Self {
            origin_channel: Some(channel),
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimestampMs(i64);

impl TimestampMs {
    #[must_use]
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        )
    }

    #[must_use]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ActorDestination(Arc<dyn Send + Sync>);

impl std::fmt::Debug for ActorDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ActorDestination").finish()
    }
}

impl ActorDestination {
    #[allow(dead_code)]
    pub fn new<T: Send + Sync + 'static>(inner: T) -> Self {
        Self(Arc::new(inner))
    }

    #[allow(dead_code)]
    pub fn downcast<T: Send + Sync + 'static>(&self) -> Option<&T> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct RoutingDestination {
    pub destination: ActorDestination,
    pub registered_at: TimestampMs,
    pub is_active: bool,
}

impl RoutingDestination {
    #[must_use]
    pub fn new(destination: ActorDestination) -> Self {
        Self {
            destination,
            registered_at: TimestampMs::now(),
            is_active: true,
        }
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    pub fn activate(&mut self) {
        self.is_active = true;
    }
}

#[derive(Debug, Clone)]
pub struct ChannelEntry {
    pub channel_id: ChannelId,
    pub destinations: Vec<RoutingDestination>,
    pub broadcast_enabled: bool,
    pub created_at: TimestampMs,
}

impl ChannelEntry {
    #[must_use]
    pub fn new(channel_id: ChannelId, destination: RoutingDestination) -> Self {
        Self {
            channel_id,
            destinations: vec![destination],
            broadcast_enabled: true,
            created_at: TimestampMs::now(),
        }
    }

    pub fn add_destination(
        &mut self,
        destination: RoutingDestination,
        max_destinations: usize,
    ) -> Result<(), crate::message_router::calc::RouteError> {
        use crate::message_router::calc::RouteError;
        if self.destinations.len() >= max_destinations {
            return Err(RouteError::MaxDestinationsExceeded {
                channel_id: self.channel_id.clone(),
                max: max_destinations,
            });
        }
        self.destinations.push(destination);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn remove_destination(&mut self, index: usize) -> Option<RoutingDestination> {
        if index < self.destinations.len() {
            Some(self.destinations.remove(index))
        } else {
            None
        }
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.destinations.iter().filter(|d| d.is_active).count()
    }

    #[must_use]
    pub fn has_active(&self) -> bool {
        self.destinations.iter().any(|d| d.is_active)
    }
}
