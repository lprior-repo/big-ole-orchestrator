use vo_types::{InstanceId, SpawnId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnRecord {
    pub spawn_id: Option<SpawnId>,
    pub instance_id: InstanceId,
    pub command: String,
    pub spawn_phase: SpawnPhase,
    pub health_checks: u32,
    pub spawn_attempts: u32,
    pub last_error: Option<super::SpawnSupervisorError>,
}

impl SpawnRecord {
    pub fn new(
        instance_id: InstanceId,
        command: String,
        spawn_id: Option<SpawnId>,
    ) -> Self {
        Self {
            spawn_id,
            instance_id,
            command,
            spawn_phase: SpawnPhase::Spawn,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: None,
        }
    }

    pub fn transition_to_health_check(&self) -> Self {
        Self {
            spawn_phase: SpawnPhase::HealthCheck,
            ..self.clone()
        }
    }

    pub fn transition_to_running(&self) -> Self {
        Self {
            spawn_phase: SpawnPhase::Running,
            ..self.clone()
        }
    }

    pub fn transition_to_shutdown(&self) -> Self {
        Self {
            spawn_phase: SpawnPhase::Shutdown,
            ..self.clone()
        }
    }

    pub fn respawn(&self, new_spawn_id: Option<SpawnId>) -> Self {
        Self {
            spawn_id: new_spawn_id,
            instance_id: self.instance_id.clone(),
            command: self.command.clone(),
            spawn_phase: SpawnPhase::Spawn,
            health_checks: 0,
            spawn_attempts: self.spawn_attempts.saturating_add(1),
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnPhase {
    Spawn,
    HealthCheck,
    Running,
    Shutdown,
    Terminated,
    Failed,
}

impl std::fmt::Display for SpawnPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn => write!(f, "spawn"),
            Self::HealthCheck => write!(f, "health-check"),
            Self::Running => write!(f, "running"),
            Self::Shutdown => write!(f, "shutdown"),
            Self::Terminated => write!(f, "terminated"),
            Self::Failed => write!(f, "failed"),
        }
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
    fn spawn_record_new_sets_correct_defaults() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), "./worker".to_string(), None);

        assert_eq!(record.instance_id, instance_id);
        assert_eq!(record.command, "./worker");
        assert_eq!(record.spawn_phase, SpawnPhase::Spawn);
        assert_eq!(record.spawn_attempts, 1);
        assert_eq!(record.health_checks, 0);
        assert!(record.last_error.is_none());
    }

    #[test]
    fn spawn_record_transition_to_health_check() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, "./worker".to_string(), None);
        let transitioned = record.transition_to_health_check();

        assert_eq!(transitioned.spawn_phase, SpawnPhase::HealthCheck);
        assert_eq!(transitioned.instance_id, record.instance_id);
        assert_eq!(transitioned.command, record.command);
        assert_eq!(transitioned.spawn_attempts, record.spawn_attempts);
    }

    #[test]
    fn spawn_record_transition_to_running() {
        let instance_id = test_instance_id();
        let record =
            SpawnRecord::new(instance_id, "./worker".to_string(), None).transition_to_health_check();
        let transitioned = record.transition_to_running();

        assert_eq!(transitioned.spawn_phase, SpawnPhase::Running);
    }

    #[test]
    fn spawn_record_transition_to_shutdown() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, "./worker".to_string(), None)
            .transition_to_health_check()
            .transition_to_running();
        let transitioned = record.transition_to_shutdown();

        assert_eq!(transitioned.spawn_phase, SpawnPhase::Shutdown);
    }

    #[test]
    fn spawn_record_respawn_increments_attempts() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            command: "./worker".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 3,
            last_error: None,
        };

        let respawned = record.respawn(Some(SpawnId::new("new-spawn".to_string())));

        assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
        assert_eq!(respawned.spawn_attempts, 4);
        assert_eq!(respawned.health_checks, 0);
        assert!(respawned.last_error.is_none());
    }

    #[test]
    fn spawn_record_respawn_saturating_at_u32_max() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, "./worker".to_string(), None);
        record.spawn_phase = SpawnPhase::Failed;
        record.spawn_attempts = u32::MAX;

        let respawned = record.respawn(None);
        assert_eq!(respawned.spawn_attempts, u32::MAX);
    }

    #[test]
    fn spawn_phase_display() {
        assert_eq!(SpawnPhase::Spawn.to_string(), "spawn");
        assert_eq!(SpawnPhase::HealthCheck.to_string(), "health-check");
        assert_eq!(SpawnPhase::Running.to_string(), "running");
        assert_eq!(SpawnPhase::Shutdown.to_string(), "shutdown");
        assert_eq!(SpawnPhase::Terminated.to_string(), "terminated");
        assert_eq!(SpawnPhase::Failed.to_string(), "failed");
    }
}
