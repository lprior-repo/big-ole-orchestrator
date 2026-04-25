pub use mocks::*;
pub use helpers::*;

mod helpers {
    use vo_types::InstanceId;

    pub fn test_instance_id() -> InstanceId {
        use ulid::Ulid;
        let ulid = Ulid::new();
        InstanceId::from_bytes(ulid.to_bytes())
    }
}

mod mocks {
    use std::path::PathBuf;
    use std::sync::Arc;

    use vo_actor::spawn_supervisor::{
        ProcessHandle, ProcessManager, SpawnPhase, SpawnRecord, SpawnStorage, SpawnSupervisorError,
        WorkQueue,
    };
    use vo_types::InstanceId;

    #[derive(Debug, Default)]
    pub struct MockSpawnStorage {
        pub records: std::sync::Mutex<Vec<SpawnRecord>>,
        pub should_fail: std::sync::Mutex<bool>,
        pub save_error: std::sync::Mutex<Option<SpawnSupervisorError>>,
    }

    impl MockSpawnStorage {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn set_save_error(&self, err: SpawnSupervisorError) {
            *self.save_error.lock().unwrap() = Some(err);
        }

        pub fn add_record(&self, record: SpawnRecord) {
            self.records.lock().unwrap().push(record);
        }

        pub fn get_records(&self) -> Vec<SpawnRecord> {
            self.records.lock().unwrap().clone()
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }
    }

    #[async_trait::async_trait]
    impl SpawnStorage for MockSpawnStorage {
        async fn get_spawn_record(&self, instance_id: &InstanceId) -> Option<SpawnRecord> {
            self.records
                .lock()
                .unwrap()
                .iter()
                .find(|r| r.instance_id == *instance_id)
                .cloned()
        }

        async fn save_spawn_record(&self, record: &SpawnRecord) -> Result<(), SpawnSupervisorError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SpawnSupervisorError::StorageError(
                    "Mock storage failure".to_string(),
                ));
            }
            if let Some(err) = self.save_error.lock().unwrap().take() {
                return Err(err);
            }
            let mut records = self.records.lock().unwrap();
            if let Some(pos) = records
                .iter()
                .position(|r| r.instance_id == record.instance_id)
            {
                records[pos] = record.clone();
            } else {
                records.push(record.clone());
            }
            Ok(())
        }

        async fn delete_spawn_record(
            &self,
            instance_id: &InstanceId,
        ) -> Result<(), SpawnSupervisorError> {
            let mut records = self.records.lock().unwrap();
            records.retain(|r| r.instance_id != *instance_id);
            Ok(())
        }

        async fn scan_spawns_by_phase(&self, phase: SpawnPhase, _max: u32) -> Vec<SpawnRecord> {
            self.records
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.spawn_phase == phase)
                .cloned()
                .collect()
        }

        async fn transition_phase(
            &self,
            instance_id: &InstanceId,
            new_phase: SpawnPhase,
        ) -> Result<(), SpawnSupervisorError> {
            let mut records = self.records.lock().unwrap();
            if let Some(record) = records.iter_mut().find(|r| r.instance_id == *instance_id) {
                record.spawn_phase = new_phase;
                Ok(())
            } else {
                Err(SpawnSupervisorError::InstanceNotFound(instance_id.clone()))
            }
        }
    }

    #[derive(Debug)]
    pub struct MockProcessManager {
        pub should_fail: std::sync::Mutex<bool>,
        pub spawn_error: std::sync::Mutex<Option<SpawnSupervisorError>>,
        pub health_check_result: std::sync::Mutex<Result<bool, SpawnSupervisorError>>,
        pub zombie_result: std::sync::Mutex<Result<bool, SpawnSupervisorError>>,
        pub terminated_pids: std::sync::Mutex<Vec<u32>>,
    }

    impl MockProcessManager {
        pub fn new() -> Self {
            Self {
                should_fail: std::sync::Mutex::new(false),
                spawn_error: std::sync::Mutex::new(None),
                health_check_result: std::sync::Mutex::new(Ok(true)),
                zombie_result: std::sync::Mutex::new(Ok(false)),
                terminated_pids: std::sync::Mutex::new(Vec::new()),
            }
        }

        pub fn set_spawn_error(&self, err: SpawnSupervisorError) {
            *self.spawn_error.lock().unwrap() = Some(err);
        }

        pub fn set_health_check_result(&self, result: Result<bool, SpawnSupervisorError>) {
            *self.health_check_result.lock().unwrap() = result;
        }

        pub fn set_zombie_result(&self, result: Result<bool, SpawnSupervisorError>) {
            *self.zombie_result.lock().unwrap() = result;
        }

        pub fn get_terminated_pids(&self) -> Vec<u32> {
            self.terminated_pids.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl ProcessManager for MockProcessManager {
        async fn spawn_process(
            &self,
            executable: &std::path::Path,
            args: &[String],
        ) -> Result<ProcessHandle, SpawnSupervisorError> {
            if let Some(err) = self.spawn_error.lock().unwrap().take() {
                return Err(err);
            }
            Ok(ProcessHandle::new(1234, executable.to_path_buf(), args.to_vec()))
        }

        async fn check_health(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
            self.health_check_result.lock().unwrap().clone()
        }

        async fn is_zombie(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
            self.zombie_result.lock().unwrap().clone()
        }

        async fn terminate(&self, pid: u32) -> Result<(), SpawnSupervisorError> {
            self.terminated_pids.lock().unwrap().push(pid);
            Ok(())
        }

        async fn wait(&self, _pid: u32) -> Result<i32, SpawnSupervisorError> {
            Ok(0)
        }
    }

    #[derive(Debug, Default)]
    pub struct MockWorkQueue {
        pub enqueued_spawns: std::sync::Mutex<Vec<InstanceId>>,
        pub enqueued_resumes: std::sync::Mutex<Vec<InstanceId>>,
        pub should_fail: std::sync::Mutex<bool>,
    }

    impl MockWorkQueue {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn get_enqueued_spawns(&self) -> Vec<InstanceId> {
            self.enqueued_spawns.lock().unwrap().clone()
        }

        pub fn get_enqueued_resumes(&self) -> Vec<InstanceId> {
            self.enqueued_resumes.lock().unwrap().clone()
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }
    }

    #[async_trait::async_trait]
    impl WorkQueue for MockWorkQueue {
        async fn enqueue_spawn(
            &self,
            instance_id: InstanceId,
            _executable: PathBuf,
            _args: Vec<String>,
        ) -> Result<(), SpawnSupervisorError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SpawnSupervisorError::DispatchError(
                    "Queue full".to_string(),
                ));
            }
            self.enqueued_spawns.lock().unwrap().push(instance_id);
            Ok(())
        }

        async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SpawnSupervisorError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SpawnSupervisorError::DispatchError(
                    "Queue full".to_string(),
                ));
            }
            self.enqueued_resumes.lock().unwrap().push(instance_id);
            Ok(())
        }
    }
}
