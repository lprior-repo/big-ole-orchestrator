//! wtf-common — shared types for wtf-engine.
//!
//! The central type is [`WorkflowEvent`] — the only type written to NATS `JetStream`.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub mod events;
pub mod types;

pub use events::{EffectDeclaration, RetryPolicy, WorkflowEvent};
pub use types::*;
