use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

use crate::{InstanceId, ParseError, TimerId};

#[derive(Debug)]
struct CounterEntry {
    counter: AtomicU64,
}

#[derive(Debug, Default)]
pub struct TimerIdGenerator {
    counters: std::sync::Mutex<std::collections::HashMap<InstanceId, Arc<CounterEntry>>>,
}

impl TimerIdGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_counter(&self, instance_id: &InstanceId) -> Arc<CounterEntry> {
        let mut counters = self.counters.lock().unwrap();
        counters
            .entry(instance_id.clone())
            .or_insert_with(|| Arc::new(CounterEntry { counter: AtomicU64::new(0) }))
            .clone()
    }

    pub fn generate(
        &self,
        instance_id: &InstanceId,
        step_index: u64,
        timestamp_ms: u64,
    ) -> TimerId {
        let entry = self.get_counter(instance_id);
        let counter = entry.counter.fetch_add(1, Ordering::SeqCst);

        let combined = format!(
            "{}-{}-{}",
            instance_id.as_str(),
            step_index,
            timestamp_ms
        );

        let uuid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, combined.as_bytes());
        let with_counter = format!("{}-{}", uuid.to_string(), counter);

        TimerId(with_counter)
    }

    #[cfg(test)]
    pub fn generate_for_testing(
        &self,
        instance_id: &InstanceId,
        step_index: u64,
        timestamp_ms: u64,
    ) -> TimerId {
        self.generate(instance_id, step_index, timestamp_ms)
    }

    #[cfg(test)]
    pub fn counters_len(&self) -> usize {
        self.counters.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task::JoinSet;

    #[test]
    fn timer_id_generator_produces_unique_ids() {
        let generator = TimerIdGenerator::new();
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let timestamp_ms = 1234567890u64;

        let id1 = generator.generate(&instance_id, 1, timestamp_ms);
        let id2 = generator.generate(&instance_id, 1, timestamp_ms);

        assert_ne!(id1, id2, "Two calls should produce different IDs");
    }

    #[test]
    fn timer_id_generator_different_steps_produce_different_ids() {
        let generator = TimerIdGenerator::new();
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let timestamp_ms = 1234567890u64;

        let id1 = generator.generate(&instance_id, 1, timestamp_ms);
        let id2 = generator.generate(&instance_id, 2, timestamp_ms);

        assert_ne!(id1, id2, "Different steps should produce different IDs");
    }

    #[test]
    fn timer_id_generator_different_timestamps_produce_different_ids() {
        let generator = TimerIdGenerator::new();
        let instance_id = InstanceId::from_bytes([1u8; 16]);

        let id1 = generator.generate(&instance_id, 1, 1000);
        let id2 = generator.generate(&instance_id, 1, 2000);

        assert_ne!(id1, id2, "Different timestamps should produce different IDs");
    }

    #[test]
    fn timer_id_generator_different_instances_produce_different_ids() {
        let generator = TimerIdGenerator::new();
        let instance_id1 = InstanceId::from_bytes([1u8; 16]);
        let instance_id2 = InstanceId::from_bytes([2u8; 16]);
        let timestamp_ms = 1234567890u64;

        let id1 = generator.generate(&instance_id1, 1, timestamp_ms);
        let id2 = generator.generate(&instance_id2, 1, timestamp_ms);

        assert_ne!(id1, id2, "Different instances should produce different IDs");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn timer_id_generator_concurrent_no_collisions() {
        let generator = Arc::new(TimerIdGenerator::new());
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let timestamp_ms = 1234567890u64;

        let mut join_set = JoinSet::new();
        let timers_per_thread = 1250;
        let num_threads = 8;

        for thread_id in 0..num_threads {
            let generator_clone = generator.clone();
            let instance_id_clone = instance_id.clone();
            join_set.spawn(async move {
                let mut ids = Vec::with_capacity(timers_per_thread);
                for i in 0..timers_per_thread {
                    let step_index = (thread_id * timers_per_thread + i) as u64;
                    let id = generator_clone.generate(&instance_id_clone, step_index, timestamp_ms);
                    ids.push(id);
                }
                ids
            });
        }

        let mut all_ids = Vec::with_capacity(timers_per_thread * num_threads);
        while let Some(result) = join_set.join_next().await {
            all_ids.extend(result.unwrap());
        }

        assert_eq!(
            all_ids.len(),
            timers_per_thread * num_threads,
            "Should have generated all timers"
        );

        let mut unique_ids = all_ids.clone();
        unique_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        unique_ids.dedup();

        assert_eq!(
            all_ids.len(),
            unique_ids.len(),
            "No duplicate TimerIds should be generated. Total: {}, Unique: {}",
            all_ids.len(),
            unique_ids.len()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn timer_id_generator_concurrent_same_inputs_no_collisions() {
        let generator = Arc::new(TimerIdGenerator::new());
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let timestamp_ms = 1234567890u64;
        let step_index = 42u64;

        let mut join_set = JoinSet::new();
        let timers_per_thread = 1250;
        let num_threads = 8;

        for _ in 0..num_threads {
            let generator_clone = generator.clone();
            let instance_id_clone = instance_id.clone();
            join_set.spawn(async move {
                let mut ids = Vec::with_capacity(timers_per_thread);
                for _ in 0..timers_per_thread {
                    let id =
                        generator_clone.generate(&instance_id_clone, step_index, timestamp_ms);
                    ids.push(id);
                }
                ids
            });
        }

        let mut all_ids = Vec::with_capacity(timers_per_thread * num_threads);
        while let Some(result) = join_set.join_next().await {
            all_ids.extend(result.unwrap());
        }

        let total = timers_per_thread * num_threads;
        assert_eq!(
            all_ids.len(),
            total,
            "Should have generated {} timers",
            total
        );

        let mut unique_ids = all_ids.clone();
        unique_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        unique_ids.dedup();

        assert_eq!(
            all_ids.len(),
            unique_ids.len(),
            "No duplicate TimerIds should be generated even with same instance/step/timestamp. Total: {}, Unique: {}",
            all_ids.len(),
            unique_ids.len()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn timer_id_generator_concurrent_10000_timers_8_threads() {
        let generator = Arc::new(TimerIdGenerator::new());
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let timestamp_ms = 1234567890u64;

        let mut join_set = JoinSet::new();
        let timers_per_thread = 1250;
        let num_threads = 8;

        for thread_id in 0..num_threads {
            let generator_clone = generator.clone();
            let instance_id_clone = instance_id.clone();
            join_set.spawn(async move {
                let mut ids = Vec::with_capacity(timers_per_thread);
                for i in 0..timers_per_thread {
                    let step_index = (thread_id * timers_per_thread + i) as u64;
                    let id = generator_clone.generate(&instance_id_clone, step_index, timestamp_ms);
                    ids.push(id);
                }
                ids
            });
        }

        let mut all_ids = Vec::with_capacity(10000);
        while let Some(result) = join_set.join_next().await {
            all_ids.extend(result.unwrap());
        }

        assert_eq!(all_ids.len(), 10000, "Should have generated exactly 10000 timers");

        let mut unique_ids = all_ids.clone();
        unique_ids.sort();
        unique_ids.dedup();

        assert_eq!(
            all_ids.len(),
            unique_ids.len(),
            "10000 timers from 8 threads must have ZERO duplicates. Found {} duplicates",
            all_ids.len() - unique_ids.len()
        );
    }
}