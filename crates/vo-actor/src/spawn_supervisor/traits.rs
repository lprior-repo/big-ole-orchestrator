use async_trait::async_trait;
use vo_types::InstanceId;

use super::process::ProcessHandle;
use super::{SpawnPhase, SpawnRecord, SpawnSupervisorError};

#[async_trait]
pub trait SpawnStorage: Send + Sync {
    async fn get_spawn_record(&self, instance_id: &InstanceId) -> Option<SpawnRecord>;

    async fn save_spawn_record(&self, record: &SpawnRecord) -> Result<(), SpawnSupervisorError>;

    async fn delete_spawn_record(
        &self,
        instance_id: &InstanceId,
    ) -> Result<(), SpawnSupervisorError>;

    async fn scan_spawns_by_phase(&self, phase: SpawnPhase, max: u32) -> Vec<SpawnRecord>;

    async fn transition_phase(
        &self,
        instance_id: &InstanceId,
        new_phase: SpawnPhase,
    ) -> Result<(), SpawnSupervisorError>;
}

#[async_trait]
pub trait ProcessManager: Send + Sync {
    async fn spawn_process(
        &self,
        executable: &str,
        args: &[String],
    ) -> Result<ProcessHandle, SpawnSupervisorError>;

    async fn check_health(&self, pid: u32) -> Result<bool, SpawnSupervisorError>;

    async fn is_zombie(&self, pid: u32) -> Result<bool, SpawnSupervisorError>;

    async fn terminate(&self, pid: u32) -> Result<(), SpawnSupervisorError>;

    async fn wait(&self, pid: u32) -> Result<i32, SpawnSupervisorError>;
}

#[async_trait]
pub trait WorkQueue: Send + Sync {
    async fn enqueue_spawn(
        &self,
        instance_id: InstanceId,
        executable: String,
        args: Vec<String>,
    ) -> Result<(), SpawnSupervisorError>;

    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SpawnSupervisorError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_storage_is_object_safe() {
        fn _assert_send_sync<T: Send + Sync>() {}
        fn _assert_object_safe(_: &dyn SpawnStorage) {}
    }

    #[test]
    fn process_manager_is_object_safe() {
        fn _assert_object_safe(_: &dyn ProcessManager) {}
    }

    #[test]
    fn work_queue_is_object_safe() {
        fn _assert_object_safe(_: &dyn WorkQueue) {}
    }
}
