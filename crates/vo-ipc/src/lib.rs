//! Inter-process communication for the Veloxide workflow engine.
//!
//! This crate provides IPC mechanisms for subprocess management and
//! envelope-based communication between the engine and worker processes.
//!
//! # Key Modules
//!
//! - [`envelope`] - Envelope serialization for task results and errors
//! - [`config`] - Subprocess configuration
//! - [`run`] - Subprocess spawning and management
//! - [`spsc`] - Single-producer single-consumer channels
//!
//! # Protocol
//!
//! Uses file descriptor passing (fd3/fd4) for efficient zero-copy IPC.

#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![cfg_attr(not(test), deny(clippy::panic))]
#![cfg_attr(not(test), warn(clippy::pedantic))]
#![cfg_attr(not(test), warn(clippy::nursery))]

pub mod config;
pub mod envelope;
pub mod error;
pub mod run;
pub mod spsc;
pub mod stderr;

pub use config::SubprocessConfig;
pub use envelope::MAX_PAYLOAD_SIZE;
pub use envelope::{MAX_MAP_ENTRIES, MAX_MAP_VALUE_BYTES};
pub use envelope::{
    engine_receive_envelope, read_envelope, validate_identity, write_envelope, Fd3Envelope,
    Fd4Envelope, TaskError, TaskResult,
};
pub use error::{ConfigError, IpcError};
pub use run::{run_subprocess, SubprocessOutput};
pub use stderr::{MAX_STDERR_BYTES, TRUNCATION_MARKER};

#[cfg(test)]
mod red_queen_tests;
#[cfg(test)]
mod unit_tests;
