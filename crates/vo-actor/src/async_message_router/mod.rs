use std::sync::Arc;

use tokio::sync::Mutex;

use crate::message_router::{ActorDestination, ChannelId, RouteError, RouterConfig, TypedMessage};

mod port;
mod operations;
#[cfg(test)]
mod tests;

/// Async wrapper around [`MessageRouter`] with Mutex-guarded delegation.
///
/// All operations acquire the inner mutex lock before forwarding to the
/// synchronous [`crate::message_router::MessageRouter`].
#[derive(Debug, Clone)]
pub struct AsyncMessageRouter {
    inner: Arc<Mutex<crate::message_router::MessageRouter>>,
}

impl AsyncMessageRouter {
    #[must_use]
    pub fn new(config: RouterConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(crate::message_router::MessageRouter::new(config))),
        }
    }

    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(RouterConfig::default())
    }
}
