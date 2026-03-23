//! wtf_client - HTTP + SSE client for wtf-api
//! Placeholder - full implementation in wtf-mwlt bead

pub mod client;
pub mod types;
pub mod watch;

pub use watch::{use_instance_watch, watch_namespace, WatchError};
