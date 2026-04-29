//! Actor supervisor module for panic recovery in ractor actors.
//!
//! This module provides panic recovery capabilities for ractor actors:
//! - Panic catching with full backtrace logging
//! - Actor restart from last known good state
//! - Metrics emission for monitoring
//! - Audit logging for compliance
//! - Cascading failure prevention via actor isolation
//!
//! # Architecture
//!
//! The actor supervisor follows the Data → Calc → Actions pattern:
//!
//! - **Data**: Types, metrics, audit entries
//! - **Calc**: Restart decisions, panic info extraction
//! - **Actions**: Panic catching, logging, metric emission
//!
//! # Usage
//!
//! ```ignore
//! use vo_actor::actor_supervisor::{ActorSupervisor, ActorSupervisorConfig};
//!
//! let supervisor = ActorSupervisor::new(config);
//! let handle = supervisor.spawn();
//! ```

pub mod audit;
pub mod metrics;
pub mod panic_catcher;
pub mod types;

pub use audit::{
    ActorSupervisorAuditEntry, ActorSupervisorAuditDetails, ActorSupervisorEventType,
    AuditLog, NoOpAuditLog, emit_audit_log, log_audit_entry_sync,
};
pub use metrics::{ActorSupervisorMetrics, emit_actor_restart_metric, emit_actor_isolation_metric, emit_actor_panic_metric};
pub use panic_catcher::{PanicCatcher, log_panic_info, log_panic_with_backtrace};
pub use types::{
    ActorSupervisorConfig, ActorSupervisorError, ActorSupervisorState, PanicInfo,
    RestartDecision, compute_restart_decision, capture_panic_info,
};