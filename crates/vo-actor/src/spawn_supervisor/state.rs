#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnSupervisorState {
    Stopped,
    Running,
    ShuttingDown,
    ShutDown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_debug_format() {
        assert_eq!(format!("{:?}", SpawnSupervisorState::Stopped), "Stopped");
        assert_eq!(format!("{:?}", SpawnSupervisorState::Running), "Running");
        assert_eq!(format!("{:?}", SpawnSupervisorState::ShuttingDown), "ShuttingDown");
        assert_eq!(format!("{:?}", SpawnSupervisorState::ShutDown), "ShutDown");
    }

    #[test]
    fn state_eq() {
        assert_eq!(SpawnSupervisorState::Running, SpawnSupervisorState::Running);
        assert_ne!(SpawnSupervisorState::Running, SpawnSupervisorState::Stopped);
    }

    #[test]
    fn state_clone() {
        let state = SpawnSupervisorState::Running;
        let cloned = state;
        assert_eq!(cloned, state);
    }
}
