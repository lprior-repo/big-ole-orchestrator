//! Connector runtime contract tests (ADR-041).
//!
//! Tests the full connector lifecycle contract:
//! - Connector registration and capability discovery
//! - Request/response lifecycle: prepare → commit → reconcile
//! - Ambiguity routing through reconciliation (not blind retry)
//! - Idempotency-key HTTP connector with mock server
//! - SQL connector under crash injection
//! - Ambiguity detection for timeout + unknown states

use std::sync::atomic::Ordering;
use vo_worker::{CommitOutcome, ConnectorError, PreparedEffect, ReconcileOutcome};
use vo_worker::{ConnectorRegistry, HttpConnector};

pub use std::sync::atomic::Ordering;

mod test_connectors;
pub use test_connectors::*;

mod registration_tests;
mod capability_discovery_tests;
mod lifecycle_tests;
mod ambiguity_routing_tests;
mod http_connector_tests;
mod crash_injection_tests;
mod ambiguity_detection_tests;
mod integration_reconciliation_tests;
mod sql_connector_transaction_tests;