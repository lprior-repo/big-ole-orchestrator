//! Current-thread SDK runtime for vo-sdk
//!
//! Provides an ultra-lightweight single-threaded async runtime for executing
//! workflow steps without the cold-start latency of a full Tokio multi-threaded runtime.
//! See ADR-011 for details.

use std::future::Future;

#[derive(Debug, Clone)]
pub enum StartError {
    BuildFailed(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::BuildFailed(msg) => write!(f, "Failed to build runtime: {}", msg),
        }
    }
}

impl std::error::Error for StartError {}

#[cfg(feature = "tokio_unstable")]
pub mod internal {
    pub use tokio::runtime::{Builder, Handle};
}

pub fn start<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build current-thread runtime");

    runtime.block_on(future)
}

pub fn spawn_and_wait<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build current-thread runtime");

    runtime.block_on(future)
}

pub fn current_thread_runtime() -> Result<tokio::runtime::Runtime, StartError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| StartError::BuildFailed(e.to_string()))
}

pub fn in_current_thread<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build current-thread runtime");

    runtime.block_on(async { f() })
}

thread_local! {
    static RUNTIME: std::cell::Cell<Option<tokio::runtime::Handle>> = const { std::cell::Cell::new(None) };
}

pub fn with_runtime<F, R>(handle: tokio::runtime::Handle, f: F) -> R
where
    F: FnOnce() -> R,
{
    RUNTIME.with(|cell| {
        let prev = cell.replace(Some(handle));
        let result = f();
        cell.set(prev);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_runs_future_to_completion() {
        let result = start(async { 42 });
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn start_runs_async_computation() {
        let result = start(async {
            let x = 10;
            let y = 20;
            x + y
        });
        assert_eq!(result, Ok(30));
    }

    #[test]
    fn in_current_thread_runs_sync_function() {
        let result = in_current_thread(|| 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn current_thread_runtime_can_spawn_tasks() {
        let runtime = current_thread_runtime().unwrap();
        let result = runtime.block_on(async {
            tokio::spawn(async { 42 }).await.unwrap()
        });
        assert_eq!(result, 42);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn with_runtime_provides_handle() {
        let handle = tokio::runtime::Handle::current();
        let result = with_runtime(handle, || {
            tokio::spawn(async { 42 })
        })
        .await
        .unwrap()
        .await.unwrap();
        assert_eq!(result, 42);
    }
}
