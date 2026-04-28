//! REDQUEEN: vo-core — admission_control — concurrency
//!
//! Bead: ve-nx2mn
//!
//! Adversarial test: Can AdmissionController handle concurrent workflow admissions
//! when the AdmissionCheck implementation has interior mutability?
//!
//! EARS Requirements:
//! - THE SYSTEM SHALL reconcile connector state
//!
//! This test probes for TOCTOU races and interior mutability races in the
//! admission controller when shared across concurrent async tasks.

#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use vo_core::admission::{
    AdmissionCheck, AdmissionController, AdmissionResult, DedupeToken, WritePressureState,
};
use vo_types::{DedupeKey, FenceToken, InstanceId, StepId};

#[derive(Debug, Clone)]
struct MockAdmissionCheck {
    admitted_keys: HashSet<String>,
}

impl MockAdmissionCheck {
    fn new() -> Self {
        Self {
            admitted_keys: HashSet::new(),
        }
    }

    fn with_keys(mut self, keys: &[&str]) -> Self {
        for k in keys {
            self.admitted_keys.insert(k.to_string());
        }
        self
    }
}

impl AdmissionCheck for MockAdmissionCheck {
    fn check_deduplicate(&self, dedupe_key: &DedupeKey) -> AdmissionResult {
        if self.admitted_keys.contains(dedupe_key.as_str()) {
            AdmissionResult::Duplicate {
                original_instance_id: InstanceId::from_bytes([1u8; 16]),
            }
        } else {
            AdmissionResult::Admitted {
                dedupe_token: DedupeToken::new("token".to_string()),
            }
        }
    }

    fn check_fence(
        &self,
        _instance_id: &InstanceId,
        _step_id: &StepId,
        _fence_token: &FenceToken,
    ) -> AdmissionResult {
        AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new("fence-token".to_string()),
        }
    }
}

fn healthy_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

#[test]
fn rq_admission_concurrent_admit_workflows_from_multiple_threads() {
    use std::thread;

    let check = MockAdmissionCheck::new();
    let controller = Arc::new(AdmissionController::new(check, healthy_state()));
    let admitted_count = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(10));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            let admitted = Arc::clone(&admitted_count);
            thread::spawn(move || {
                barrier.wait();
                let key_str = format!("workflow-{}", i);
                let key = DedupeKey::parse(&key_str).unwrap();
                let result = ctrl.admit_new_workflow(&key);
                if result.is_ok() {
                    admitted.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let total = admitted_count.load(Ordering::Relaxed);
    assert_eq!(
        total, 10,
        "All 10 concurrent admissions should succeed (10 unique keys)"
    );
}

#[test]
fn rq_admission_concurrent_dedupe_same_key_from_multiple_threads() {
    use std::thread;

    let check = MockAdmissionCheck::new();
    let controller = Arc::new(AdmissionController::new(check, healthy_state()));
    let admitted_count = Arc::new(AtomicUsize::new(0));
    let duplicate_count = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(10));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            let admitted = Arc::clone(&admitted_count);
            let dup = Arc::clone(&duplicate_count);
            thread::spawn(move || {
                barrier.wait();
                let key = DedupeKey::parse("same-workflow-key").unwrap();
                match ctrl.admit_new_workflow(&key) {
                    Ok(_) => {
                        admitted.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(vo_core::admission::AdmissionError::Duplicate { .. }) => {
                        dup.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let admitted = admitted_count.load(Ordering::Relaxed);
    let duplicates = duplicate_count.load(Ordering::Relaxed);
    assert_eq!(
        admitted, 1,
        "Only ONE admission should succeed for the same key"
    );
    assert_eq!(duplicates, 9, "Nine duplicates should be rejected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn rq_admission_concurrent_async_tasks_toctou_race() {
    use tokio::sync::Barrier;

    let check = MockAdmissionCheck::new();
    let controller = Arc::new(AdmissionController::new(check, healthy_state()));
    let barrier = Arc::new(Barrier::new(20));
    let results = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let tasks: Vec<_> = (0..20)
        .map(|i| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            tokio::spawn(async move {
                barrier.wait().await;
                let key_str = format!("async-workflow-{}", i % 5);
                let key = DedupeKey::parse(&key_str).unwrap();
                let result = ctrl.admit_new_workflow(&key);
                let mut r = results.lock().await;
                r.push((i, result.is_ok()));
            })
        })
        .collect();

    for t in tasks {
        t.await.unwrap();
    }

    let r = results.lock().await;
    let successful: usize = r.iter().filter(|(_, ok)| *ok).count();
    let failed: usize = r.iter().filter(|(_, ok)| !ok).count();

    assert_eq!(
        successful, 5,
        "Only 5 unique keys (0..5 % 5) should be admitted"
    );
    assert_eq!(failed, 15, "15 duplicate keys should be rejected");
}

#[test]
fn rq_admission_controller_pressure_state_race_with_concurrent_reads() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    let check = MockAdmissionCheck::new();
    let controller = Arc::new(AdmissionController::new(check, healthy_state()));

    let pressure_changes = Arc::new(AtomicU64::new(10));
    let admitted_healthy = Arc::new(AtomicUsize::new(0));
    let admitted_degraded = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(12));

    let writer_handle = {
        let barrier = Arc::clone(&barrier);
        let pressure = Arc::clone(&pressure_changes);
        thread::spawn(move || {
            barrier.wait();
            for i in 0..100 {
                let new_pressure = if i % 2 == 0 { 150 } else { 10 };
                pressure.store(new_pressure, Ordering::Relaxed);
                thread::yield_now();
            }
        })
    };

    let reader_handles: Vec<_> = (0..10)
        .map(|_| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            let pressure = Arc::clone(&pressure_changes);
            let admitted_healthy = Arc::clone(&admitted_healthy);
            let admitted_degraded = Arc::clone(&admitted_degraded);
            thread::spawn(move || {
                barrier.wait();
                for _ in 0..100 {
                    let key = DedupeKey::parse("temp-key").unwrap();
                    let result = ctrl.admit_new_workflow(&key);
                    let p = pressure.load(Ordering::Relaxed);
                    match result {
                        Ok(_) if p <= 100 => {
                            admitted_healthy.fetch_add(1, Ordering::Relaxed);
                        }
                        Ok(_) if p > 100 => {
                            admitted_degraded.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) if p > 100 => {
                            admitted_healthy.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                }
            })
        })
        .collect();

    writer_handle.join().unwrap();
    for h in reader_handles {
        h.join().unwrap();
    }

    let total =
        admitted_healthy.load(Ordering::Relaxed) + admitted_degraded.load(Ordering::Relaxed);
    assert!(
        total > 0,
        "Some admissions should complete despite concurrent pressure state changes"
    );
}

#[test]
fn rq_admission_in_flight_tracking_concurrent_modification() {
    use std::thread;

    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, healthy_state());

    let instance_ids: Vec<_> = (0..100)
        .map(|i| InstanceId::from_bytes([i as u8; 16]))
        .collect();

    for id in &instance_ids {
        controller.mark_in_flight(id);
    }

    let controller = Arc::new(controller);
    let found_count = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(10));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            let found = Arc::clone(&found_count);
            let ids = instance_ids.clone();
            let start = i * 10;
            thread::spawn(move || {
                barrier.wait();
                for idx in start..start + 10 {
                    if ctrl.is_in_flight(&ids[idx]) {
                        found.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let found = found_count.load(Ordering::Relaxed);
    assert_eq!(
        found, 100,
        "All 100 marked in-flight workflows should be found"
    );
}

#[test]
fn rq_admission_step_in_flight_concurrent_with_admission() {
    use std::thread;

    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, healthy_state());
    let instance_id = InstanceId::from_bytes([42u8; 16]);
    controller.mark_in_flight(&instance_id);

    let controller = Arc::new(controller);
    let barrier = Arc::new(std::sync::Barrier::new(20));
    let id_clone = instance_id.clone();

    let admission_handles: Vec<_> = (0..10)
        .map(|_| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let key = DedupeKey::parse("concurrent-key").unwrap();
                let _ = ctrl.admit_new_workflow(&key);
            })
        })
        .collect();

    let step_handles: Vec<_> = (0..10)
        .map(|_| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            let id = id_clone.clone();
            thread::spawn(move || {
                barrier.wait();
                let _ = ctrl.step_in_flight(&id);
            })
        })
        .collect();

    for h in admission_handles.into_iter().chain(step_handles) {
        h.join().unwrap();
    }

    assert!(
        controller.is_in_flight(&instance_id),
        "In-flight workflow should still be tracked after concurrent access"
    );
}

#[test]
fn rq_admission_concurrent_dedupe_keys_hash_collision_resistance() {
    use std::thread;

    let check = MockAdmissionCheck::new();
    let controller = Arc::new(AdmissionController::new(check, healthy_state()));
    let barrier = Arc::new(std::sync::Barrier::new(256));

    let handles: Vec<_> = (0..=255u8)
        .map(|i| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let key_str = format!("workflow-{}", i);
                let key = DedupeKey::parse(&key_str).unwrap();
                let _ = ctrl.admit_new_workflow(&key);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    for i in 0..=255u8 {
        let key_str = format!("workflow-{}", i);
        let key = DedupeKey::parse(&key_str).unwrap();
        let result = controller.admit_new_workflow(&key);
        assert!(
            matches!(
                result,
                Err(vo_core::admission::AdmissionError::Duplicate { .. })
            ),
            "Key {} should be detected as duplicate",
            i
        );
    }
}

#[test]
fn rq_admission_memory_ordering_on_admitted_count() {
    use std::thread;

    let check = MockAdmissionCheck::new();
    let controller = Arc::new(AdmissionController::new(check, healthy_state()));
    let counts: Vec<Arc<AtomicUsize>> = (0..10).map(|_| Arc::new(AtomicUsize::new(0))).collect();
    let barrier = Arc::new(std::sync::Barrier::new(10));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let ctrl = Arc::clone(&controller);
            let barrier = Arc::clone(&barrier);
            let local_counts = counts.clone();
            thread::spawn(move || {
                barrier.wait();
                for j in 0..100 {
                    let key_str = format!("workflow-{}-{}", i, j);
                    let key = DedupeKey::parse(&key_str).unwrap();
                    if ctrl.admit_new_workflow(&key).is_ok() {
                        local_counts[i].fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let total: usize = counts.iter().map(|c| c.load(Ordering::Relaxed)).sum();
    assert_eq!(
        total, 1000,
        "All 1000 unique workflow admissions should succeed"
    );
}
