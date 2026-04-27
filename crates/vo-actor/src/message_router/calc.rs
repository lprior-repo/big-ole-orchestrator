//! Calculation Layer — Pure Routing Decisions.
//!
//! Pure functions for routing logic: error types, destination selection,
//! broadcast decisions, route validation.

use thiserror::Error;

use super::data::{ChannelEntry, ChannelId, RouterConfig, RoutingDestination};

#[derive(Debug, Clone, Error)]
pub enum RouteError {
    #[error("channel not found: {0}")]
    ChannelNotFound(ChannelId),

    #[error("no active destinations for channel: {0}")]
    NoActiveDestinations(ChannelId),

    #[error("max destinations exceeded for channel {channel_id}: {max}")]
    MaxDestinationsExceeded { channel_id: ChannelId, max: usize },

    #[error("delivery timeout for channel: {0}")]
    DeliveryTimeout(ChannelId),

    #[error("actor error on channel {0}: {1}")]
    ActorError(ChannelId, String),

    #[error("dead letter queue is full")]
    DeadLetterQueueFull,

    #[error("channel already exists: {0}")]
    ChannelAlreadyExists(ChannelId),

    #[error("destination already registered for channel: {0}")]
    DestinationAlreadyRegistered(ChannelId),

    #[error("channel is closed: {0}")]
    ChannelClosed(ChannelId),
}

pub fn select_active_destinations(channel: &ChannelEntry) -> Vec<(usize, &RoutingDestination)> {
    channel
        .destinations
        .iter()
        .enumerate()
        .filter(|(_, d)| d.is_active)
        .collect()
}

pub fn should_broadcast(channel: &ChannelEntry, config: &RouterConfig) -> bool {
    config.broadcast_enabled && channel.broadcast_enabled && channel.destinations.len() > 1
}

pub fn validate_route(
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
