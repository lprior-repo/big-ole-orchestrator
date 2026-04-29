use std::collections::HashMap;

use crate::error::SchedulerError;
use crate::types::job::ScheduledJob;
use crate::types::JobId;

pub trait SchedulerStore {
    fn put(&mut self, job: ScheduledJob) -> Result<(), SchedulerError>;
    fn get(&self, id: &JobId) -> Result<Option<ScheduledJob>, SchedulerError>;
    fn remove(&mut self, id: &JobId) -> Result<Option<ScheduledJob>, SchedulerError>;
    fn update(&mut self, job: ScheduledJob) -> Result<(), SchedulerError>;
    fn list_all(&self) -> Result<Vec<ScheduledJob>, SchedulerError>;
    fn contains(&self, id: &JobId) -> Result<bool, SchedulerError>;
}

pub struct InMemorySchedulerStore {
    pub serialized: HashMap<Vec<u8>, Vec<u8>>,
}

impl InMemorySchedulerStore {
    pub fn new() -> Self {
        Self {
            serialized: HashMap::new(),
        }
    }

    fn serialize_job(job: &ScheduledJob) -> Result<Vec<u8>, SchedulerError> {
        serde_json::to_vec(job).map_err(|e| SchedulerError::SerializationError(e.to_string()))
    }

    fn deserialize_job(data: &[u8]) -> Result<ScheduledJob, SchedulerError> {
        serde_json::from_slice(data).map_err(|e| SchedulerError::SerializationError(e.to_string()))
    }

    fn key(id: &JobId) -> Vec<u8> {
        id.0.to_bytes().to_vec()
    }
}

impl Default for InMemorySchedulerStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulerStore for InMemorySchedulerStore {
    fn put(&mut self, job: ScheduledJob) -> Result<(), SchedulerError> {
        let key = Self::key(&job.id);
        let value = Self::serialize_job(&job)?;
        self.serialized.insert(key, value);
        Ok(())
    }

    fn get(&self, id: &JobId) -> Result<Option<ScheduledJob>, SchedulerError> {
        let key = Self::key(id);
        match self.serialized.get(&key) {
            Some(data) => Ok(Some(Self::deserialize_job(data)?)),
            None => Ok(None),
        }
    }

    fn remove(&mut self, id: &JobId) -> Result<Option<ScheduledJob>, SchedulerError> {
        let key = Self::key(id);
        match self.serialized.remove(&key) {
            Some(data) => Ok(Some(Self::deserialize_job(&data)?)),
            None => Ok(None),
        }
    }

    fn update(&mut self, job: ScheduledJob) -> Result<(), SchedulerError> {
        let key = Self::key(&job.id);
        if !self.serialized.contains_key(&key) {
            return Err(SchedulerError::JobNotFound);
        }
        let value = Self::serialize_job(&job)?;
        self.serialized.insert(key, value);
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<ScheduledJob>, SchedulerError> {
        self.serialized
            .values()
            .map(|data| Self::deserialize_job(data))
            .collect()
    }

    fn contains(&self, id: &JobId) -> Result<bool, SchedulerError> {
        let key = Self::key(id);
        Ok(self.serialized.contains_key(&key))
    }
}

#[cfg(test)]
mod store_tests {
    use super::*;
    use crate::types::{JobKind, JobPriority, RetryPolicy, SchedulePolicy};

    fn make_job() -> ScheduledJob {
        ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy::default(),
            bytes::Bytes::from_static(b"test-payload"),
        )
        .unwrap()
    }

    #[test]
    fn put_and_get_roundtrip() {
        let mut store = InMemorySchedulerStore::new();
        let job = make_job();
        let id = job.id;
        store.put(job).unwrap();
        let retrieved = store.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.id, id);
    }

    #[test]
    fn get_returns_none_for_missing() {
        let store = InMemorySchedulerStore::new();
        let result = store.get(&JobId::generate()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn remove_returns_job() {
        let mut store = InMemorySchedulerStore::new();
        let job = make_job();
        let id = job.id;
        store.put(job).unwrap();
        let removed = store.remove(&id).unwrap().unwrap();
        assert_eq!(removed.id, id);
        assert!(store.get(&id).unwrap().is_none());
    }

    #[test]
    fn update_persists_changes() {
        let mut store = InMemorySchedulerStore::new();
        let job = make_job();
        let id = job.id;
        store.put(job).unwrap();
        let mut retrieved = store.get(&id).unwrap().unwrap();
        retrieved
            .transition(crate::types::JobState::Running)
            .unwrap();
        store.update(retrieved).unwrap();
        let after = store.get(&id).unwrap().unwrap();
        assert_eq!(after.state, crate::types::JobState::Running);
    }

    #[test]
    fn update_rejects_missing_job() {
        let mut store = InMemorySchedulerStore::new();
        let job = make_job();
        let result = store.update(job);
        assert!(matches!(result, Err(SchedulerError::JobNotFound)));
    }

    #[test]
    fn list_all_returns_all_jobs() {
        let mut store = InMemorySchedulerStore::new();
        let job1 = make_job();
        let job2 = make_job();
        store.put(job1).unwrap();
        store.put(job2).unwrap();
        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn contains_returns_true_for_existing() {
        let mut store = InMemorySchedulerStore::new();
        let job = make_job();
        let id = job.id;
        store.put(job).unwrap();
        assert!(store.contains(&id).unwrap());
    }

    #[test]
    fn contains_returns_false_for_missing() {
        let store = InMemorySchedulerStore::new();
        assert!(!store.contains(&JobId::generate()).unwrap());
    }

    #[test]
    fn serialized_data_survives_reopen() {
        let mut store = InMemorySchedulerStore::new();
        let job = make_job();
        let id = job.id;
        store.put(job).unwrap();

        let serialized_data = store.serialized.clone();
        let reopened = InMemorySchedulerStore {
            serialized: serialized_data,
        };

        let retrieved = reopened.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.state, crate::types::JobState::Pending);
    }
}
