//! Connector runtime contract implementation (ADR-041).

mod error;
mod http;
mod registry;
mod sql;
mod trait_def;
mod types;

pub use error::ConnectorError;
pub use http::HttpConnector;
pub use registry::ConnectorRegistry;
pub use sql::SqlConnector;
pub use trait_def::Connector;
pub use types::{CommitOutcome, PreparedEffect, ReconcileOutcome};

#[cfg(test)]
mod tests;
