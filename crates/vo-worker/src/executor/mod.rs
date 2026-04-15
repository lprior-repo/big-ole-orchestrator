//! Managed-effect execution path (ADR-030).
//!
//! Architecture: Data (ManagedEffectTask, ExecutionOutcome, ManagedEffectError)
//!             → Calc (classify_outcome)
//!             → Actions (ManagedEffectExecutor trait, default implementation).
//!
//! This module provides the **dedicated** runtime path for managed effects,
//! strictly isolated from unsafe/arbitrary activity execution (supervisor,
//! lock manager). Managed effects flow through the Connector prepare→commit
//! lifecycle with automatic reconciliation for ambiguous outcomes.

mod error;
mod port;
mod task;

pub use error::ManagedEffectError;
pub use port::ManagedEffectExecutor;
pub use task::{ExecutionOutcome, ManagedEffectTask};

#[cfg(test)]
mod tests;
