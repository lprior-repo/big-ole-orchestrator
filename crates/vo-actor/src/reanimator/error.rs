//! Error types for the Reanimator Loop.

use thiserror::Error;
use vo_types::InstanceId;

/// Classification of Reanimator errors by retry behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReanimatorErrorClass {
    Transient,
    Fatal,
}

/// Errors that can occur in the Reanimator Loop.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReanimatorError {
    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Corrupt key format: {0}")]
    CorruptKey(String),

    #[error("Atomicity violation: {0}")]
    AtomicityViolation(String),

    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("Failed to enqueue resume work: {0}")]
    EnqueueFailed(String),

    #[error("Reanimator is already running")]
    AlreadyRunning,

    #[error("Reanimator has already shut down")]
    AlreadyShutdown,
}

impl ReanimatorError {
    pub const fn classify(&self) -> ReanimatorErrorClass {
        match self {
            Self::StorageError(_)
            | Self::EnqueueFailed(_)
            | Self::AtomicityViolation(_)
            | Self::BudgetExceeded(_) => ReanimatorErrorClass::Transient,
            Self::CorruptKey(_)
            | Self::InstanceNotFound(_)
            | Self::AlreadyRunning
            | Self::AlreadyShutdown => ReanimatorErrorClass::Fatal,
        }
    }

    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(self.classify(), ReanimatorErrorClass::Transient)
    }

    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(self.classify(), ReanimatorErrorClass::Fatal)
    }
}

#[cfg(test)]
const _: () = {
    const fn assert_exhaustive_classification(e: &ReanimatorError) {
        match e.classify() {
            ReanimatorErrorClass::Transient | ReanimatorErrorClass::Fatal => {}
        }
    }

    const _TRANSIENT_VARIANTS: &[&str] = &[
        "StorageError",
        "EnqueueFailed",
        "AtomicityViolation",
        "BudgetExceeded",
    ];
    const _FATAL_VARIANTS: &[&str] = &[
        "CorruptKey",
        "InstanceNotFound",
        "AlreadyRunning",
        "AlreadyShutdown",
    ];
};
