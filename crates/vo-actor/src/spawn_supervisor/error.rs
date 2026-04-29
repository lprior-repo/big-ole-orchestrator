use std::time::Duration;

use vo_types::InstanceId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpawnSupervisorError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Corrupt spawn: {0}")]
    CorruptSpawn(String),
    #[error("Atomicity violation: {0}")]
    AtomicityViolation(String),
    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("Mailbox full: {0}")]
    MailboxFull(InstanceId),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Already running")]
    AlreadyRunning,
    #[error("Shutdown timeout: {0:?}")]
    ShutdownTimeout(Duration),
    #[error("Dispatch error: {0}")]
    DispatchError(String),
    #[error("Spawn failed for '{command}': {error}")]
    SpawnFailed { command: String, error: String },
    #[error("Health check {check_number} failed for {instance_id}: {error}")]
    HealthCheckFailed {
        instance_id: InstanceId,
        check_number: u32,
        error: String,
    },
    #[error("Zombie detected for {instance_id}: pid={pid}")]
    ZombieDetected { instance_id: InstanceId, pid: u32 },
    #[error("Process exited for {instance_id}: pid={pid}, code={exit_code}")]
    ProcessExited {
        instance_id: InstanceId,
        pid: u32,
        exit_code: i32,
    },
    #[error("Supervisor not running")]
    NotRunning,
    #[error("Supervisor already shutdown")]
    AlreadyShutdown,
}

impl SpawnSupervisorError {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::StorageError(_)
                | Self::InstanceNotFound(_)
                | Self::MailboxFull(_)
                | Self::DispatchError(_)
        )
    }

    #[must_use]
    pub fn is_resumable(&self) -> bool {
        matches!(
            self,
            Self::HealthCheckFailed { .. } | Self::ProcessExited { .. } | Self::SpawnFailed { .. }
        )
    }

    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CorruptSpawn(_) | Self::InvalidConfig(_) | Self::ZombieDetected { .. }
        )
    }

    #[must_use]
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            Self::AlreadyRunning
                | Self::AlreadyShutdown
                | Self::NotRunning
                | Self::ShutdownTimeout(_)
                | Self::AtomicityViolation(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn test_instance_id() -> InstanceId {
        let ulid = Ulid::new();
        InstanceId::from_bytes(ulid.to_bytes())
    }

    #[test]
    fn spawn_supervisor_error_is_transient() {
        assert!(SpawnSupervisorError::StorageError("test".to_string()).is_transient());
        assert!(SpawnSupervisorError::InstanceNotFound(test_instance_id()).is_transient());
        assert!(SpawnSupervisorError::MailboxFull(test_instance_id()).is_transient());
        assert!(SpawnSupervisorError::DispatchError("test".to_string()).is_transient());
        assert!(!SpawnSupervisorError::InvalidConfig("test".to_string()).is_transient());
    }

    #[test]
    fn spawn_supervisor_error_is_resumable() {
        assert!(SpawnSupervisorError::HealthCheckFailed {
            instance_id: test_instance_id(),
            check_number: 1,
            error: "test".to_string()
        }
        .is_resumable());
        assert!(SpawnSupervisorError::ProcessExited {
            instance_id: test_instance_id(),
            pid: 123,
            exit_code: 1
        }
        .is_resumable());
        assert!(SpawnSupervisorError::SpawnFailed {
            command: "test".to_string(),
            error: "test".to_string()
        }
        .is_resumable());
        assert!(!SpawnSupervisorError::StorageError("test".to_string()).is_resumable());
    }

    #[test]
    fn spawn_supervisor_error_is_fatal() {
        assert!(SpawnSupervisorError::CorruptSpawn("test".to_string()).is_fatal());
        assert!(SpawnSupervisorError::InvalidConfig("test".to_string()).is_fatal());
        assert!(SpawnSupervisorError::ZombieDetected {
            instance_id: test_instance_id(),
            pid: 123
        }
        .is_fatal());
        assert!(!SpawnSupervisorError::StorageError("test".to_string()).is_fatal());
    }

    #[test]
    fn spawn_supervisor_error_is_operational() {
        assert!(SpawnSupervisorError::AlreadyRunning.is_operational());
        assert!(SpawnSupervisorError::AlreadyShutdown.is_operational());
        assert!(SpawnSupervisorError::NotRunning.is_operational());
        assert!(SpawnSupervisorError::ShutdownTimeout(Duration::from_secs(30)).is_operational());
        assert!(SpawnSupervisorError::AtomicityViolation("test".to_string()).is_operational());
        assert!(!SpawnSupervisorError::StorageError("test".to_string()).is_operational());
    }
}