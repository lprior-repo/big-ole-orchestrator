//! Scheduler error types — re-exported from vo-common.
//!
//! The canonical error taxonomy lives in `vo_common::error`. This module
//! re-exports those types so that vo-scheduler consumers don't need to
//! depend on vo-common directly.

pub use vo_common::{ExecutionError, RetryExhaustedError, SchedulerError};
