use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;

use super::state::SpawnSupervisorState;

#[derive(Debug)]
pub struct SpawnSupervisorHandle {
    pub(crate) state_sender: watch::Sender<SpawnSupervisorState>,
    pub(crate) shutdown_trigger: broadcast::Sender<()>,
    pub(crate) task_handle: Option<JoinHandle<()>>,
}

impl SpawnSupervisorHandle {
    pub fn current_state(&self) -> SpawnSupervisorState {
        *self.state_sender.borrow()
    }

    #[tracing::instrument(skip(self))]
    pub async fn shutdown(mut self) -> Result<(), super::SpawnSupervisorError> {
        let _ = self.shutdown_trigger.send(());

        let mut receiver = self.state_sender.subscribe();
        loop {
            match receiver.changed().await {
                Ok(()) => {
                    let state = *receiver.borrow();
                    match state {
                        SpawnSupervisorState::ShutDown => break,
                        SpawnSupervisorState::ShuttingDown => continue,
                        _ => {
                            return Err(super::SpawnSupervisorError::AtomicityViolation(format!(
                                "Unexpected state during shutdown: {:?}",
                                state
                            )));
                        }
                    }
                }
                Err(_) => {
                    return Err(super::SpawnSupervisorError::AlreadyShutdown);
                }
            }
        }

        if let Some(task) = self.task_handle.take() {
            match task.await {
                Ok(()) => {}
                Err(e) => {
                    if !e.is_panic() {
                        tracing::warn!("Spawn supervisor task cancelled during shutdown");
                    } else {
                        tracing::error!("Spawn supervisor task panicked during shutdown");
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    fn create_test_handle() -> SpawnSupervisorHandle {
        let (state_sender, _) = watch::channel(SpawnSupervisorState::Running);
        let (shutdown_trigger, _) = broadcast::channel(1);
        SpawnSupervisorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: None,
        }
    }

    #[test]
    fn handle_current_state_returns_running() {
        let handle = create_test_handle();
        assert_eq!(handle.current_state(), SpawnSupervisorState::Running);
    }

    #[test]
    fn handle_debug_format_contains_fields() {
        let handle = create_test_handle();
        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("SpawnSupervisorHandle"));
    }
}
