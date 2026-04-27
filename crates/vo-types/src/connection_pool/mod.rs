//! Connection Pool Manager Types and Tests
//!
//! This module defines types for managing NATS client connections in the veloxide
//! distributed worker system.

mod errors;
mod tests;
mod types;

pub use errors::*;
pub use types::*;
