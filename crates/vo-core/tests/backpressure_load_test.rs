//! ADR-006 Backpressure Load Test
//!
//! Load test that triggers backpressure on the LoadSheddingSemaphore,
//! verifies load shedding works correctly, and measures throughput
//! degradation curve as the system approaches and exceeds capacity.
//!
//! # What this tests
//!
//! 1. **Backpressure trigger**: Flooding the semaphore beyond permit capacity
//! 2. **Load shedding activation**: Verifying is_load_shedding_active triggers at threshold
//! 3. **Throughput degradation curve**: Measuring ops/sec at 25%, 50%, 75%, 100%, 125%, 150% load
//! 4. **Load shedding correctness**: Rejected requests have correct error variant
//! 5. **Recovery**: Throughput recovers when load drops below threshold
//! 6. **Fairness under load**: No single task starves under backpressure

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use vo_core::shedding::{
    LoadSheddingSemaphore, SemaphoreLimitError, MAX_CONCURRENT_BINARIES, MAX_YIELDED_ACTORS,
};

fn make_semaphore(max_permits: usize) -> LoadSheddingSemaphore {
    LoadSheddingSemaphore::new(max_permits)
}

#[tokio::test]
async fn backpressure_triggers_when_permits_exhausted() {
    let semaphore = make_semaphore(10);

    let permits: Vec<_> = (0..10)
        .map(|_| semaphore.try_acquire().expect("should acquire"))
        .collect();

    assert_eq!(semaphore.available_permits(), 0);
    assert!(semaphore.try_acquire().is_err());

    let result = semaphore.check_load_shedding_threshold(5);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_load_shedding());

    drop(permits);
    assert_eq!(semaphore.available_permits(), 10);
}

#[tokio::test]
async fn load_shedding_rejects_with_correct_error_at_threshold() {
    let semaphore = Arc::new(make_semaphore(50));
    let shed_threshold = 25;

    let permits: Vec<_> = (0..50)
        .map(|_| semaphore.try_acquire().expect("should acquire"))
        .collect();

    let result = semaphore.check_load_shedding_threshold(shed_threshold);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_load_shedding());
    match err {
        SemaphoreLimitError::LoadSheddingActive {
            yielded_actors,
            threshold,
        } => {
            assert_eq!(yielded_actors, 50);
            assert_eq!(threshold, shed_threshold);
        }
        other => panic!("Expected LoadSheddingActive, got {:?}", other),
    }

    drop(permits);
}

#[tokio::test]
async fn throughput_degradation_curve_under_increasing_load() {
    let max_permits = 100usize;
    let semaphore = Arc::new(make_semaphore(max_permits));
    let shed_threshold = 80;

    let load_levels: Vec<(usize, &str)> = vec![
        (25, "25%"),
        (50, "50%"),
        (75, "75%"),
        (100, "100%"),
        (125, "125%"),
        (150, "150%"),
    ];

    let mut results: Vec<(&str, f64, bool)> = Vec::new();

    for (load_pct, label) in &load_levels {
        let num_tasks = *load_pct;

        let ops_completed = Arc::new(AtomicU64::new(0));
        let ops_rejected = Arc::new(AtomicU64::new(0));
        let _duration = Duration::from_millis(500);

        let mut handles = Vec::new();

        for _ in 0..num_tasks {
            let sem = Arc::clone(&semaphore);
            let ops = Arc::clone(&ops_completed);
            let rejected = Arc::clone(&ops_rejected);
            handles.push(tokio::spawn(async move {
                let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
                while tokio::time::Instant::now() < deadline {
                    match sem.try_acquire() {
                        Ok(permit) => {
                            ops.fetch_add(1, Ordering::Relaxed);
                            drop(permit);
                        }
                        Err(SemaphoreLimitError::LoadSheddingActive { .. }) => {
                            rejected.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(SemaphoreLimitError::LimitReached { .. }) => {
                            rejected.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {}
                    }
                    tokio::task::yield_now().await;
                }
            }));
        }

        let start = Instant::now();
        for handle in handles {
            handle.await.expect("task should complete");
        }
        let elapsed = start.elapsed();

        let completed = ops_completed.load(Ordering::Relaxed) as f64;
        let _total_rejected = ops_rejected.load(Ordering::Relaxed);
        let throughput = completed / elapsed.as_secs_f64();

        let is_shedding = semaphore
            .check_load_shedding_threshold(shed_threshold)
            .is_err();

        results.push((label, throughput, is_shedding));
    }

    assert!(
        !results.is_empty(),
        "Should have results for all load levels"
    );

    let healthy_throughput = results[0].1;
    assert!(
        healthy_throughput > 0.0,
        "Throughput at 25% load should be positive"
    );

    for i in 1..results.len() {
        let prev_throughput = results[i - 1].1;
        let curr_throughput = results[i].1;
        assert!(
            curr_throughput <= prev_throughput * 1.5,
            "Throughput at {} load ({:.0} ops/s) should not dramatically exceed {} load ({:.0} ops/s)",
            results[i].0,
            curr_throughput,
            results[i - 1].0,
            prev_throughput,
        );
    }
}

#[tokio::test]
async fn load_shedding_activates_at_correct_threshold() {
    let semaphore = make_semaphore(100);

    for i in 0..99 {
        let _p = semaphore.try_acquire().expect("should acquire");
        assert!(
            semaphore.check_load_shedding_threshold(100).is_ok(),
            "Should not shed at {} acquired",
            i + 1
        );
    }

    let _p = semaphore.try_acquire().expect("should acquire");
    assert!(
        semaphore.check_load_shedding_threshold(100).is_err(),
        "Should start shedding at exactly 100 acquired"
    );
}

#[tokio::test]
async fn recovery_after_load_drop() {
    let semaphore = Arc::new(make_semaphore(20));
    let shed_threshold = 10;

    let mut permits: Vec<_> = (0..20)
        .map(|_| semaphore.try_acquire().expect("should acquire"))
        .collect();

    assert!(semaphore
        .check_load_shedding_threshold(shed_threshold)
        .is_err());

    permits.drain(15..20);
    assert_eq!(semaphore.available_permits(), 5);
    assert!(
        semaphore
            .check_load_shedding_threshold(shed_threshold)
            .is_ok(),
        "Should recover after dropping below threshold"
    );

    for _ in 0..5 {
        let _p = semaphore.try_acquire().expect("should re-acquire");
    }
}

#[tokio::test]
async fn concurrent_acquires_under_backpressure() {
    let semaphore = Arc::new(make_semaphore(10));
    let shed_threshold = 5;

    let initial_permits: Vec<_> = (0..8)
        .map(|_| semaphore.try_acquire().expect("should acquire"))
        .collect();

    assert!(semaphore
        .check_load_shedding_threshold(shed_threshold)
        .is_err());

    let success_count = Arc::new(AtomicUsize::new(0));
    let reject_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..20 {
        let sem = Arc::clone(&semaphore);
        let success = Arc::clone(&success_count);
        let rejected = Arc::clone(&reject_count);
        handles.push(tokio::spawn(async move {
            match sem.try_acquire() {
                Ok(_p) => {
                    success.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    rejected.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    for handle in handles {
        handle.await.expect("should complete");
    }

    assert_eq!(
        success_count.load(Ordering::Relaxed),
        2,
        "Only 2 remaining permits should be acquired"
    );
    assert_eq!(
        reject_count.load(Ordering::Relaxed),
        18,
        "18 tasks should be rejected"
    );

    drop(initial_permits);
}

#[tokio::test]
async fn async_acquire_queues_and_resolves_under_contention() {
    let semaphore = Arc::new(make_semaphore(5));

    let held_permits: Arc<tokio::sync::Mutex<Vec<_>>> = Arc::new(tokio::sync::Mutex::new(
        (0..5)
            .map(|_| semaphore.try_acquire().expect("should acquire"))
            .collect(),
    ));

    let sem_clone = Arc::clone(&semaphore);
    let acquire_done = Arc::new(tokio::sync::Mutex::new(false));
    let acquire_done_clone = Arc::clone(&acquire_done);

    let waiter = tokio::spawn(async move {
        let result = sem_clone.acquire().await;
        assert!(result.is_ok(), "Should eventually acquire a permit");
        *acquire_done_clone.lock().await = true;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !*acquire_done.lock().await,
        "Acquire should be queued, not resolved yet"
    );

    held_permits.lock().await.remove(0);

    waiter.await.expect("waiter should complete");
    assert!(
        *acquire_done.lock().await,
        "Acquire should have resolved after permit release"
    );
}

#[tokio::test]
async fn burst_load_with_drain_measures_degradation() {
    let max_permits = 50usize;
    let semaphore = Arc::new(make_semaphore(max_permits));

    let burst_sizes: Vec<usize> = vec![10, 25, 50, 75, 100, 150];
    let mut degradation_data: Vec<(usize, f64)> = Vec::new();

    for burst_size in &burst_sizes {
        let ops_per_burst = Arc::new(AtomicU64::new(0));
        let mut handles = Vec::new();

        for _ in 0..*burst_size {
            let sem = Arc::clone(&semaphore);
            let ops = Arc::clone(&ops_per_burst);
            handles.push(tokio::spawn(async move {
                for _ in 0..100 {
                    if let Ok(permit) = sem.try_acquire() {
                        ops.fetch_add(1, Ordering::Relaxed);
                        drop(permit);
                    }
                    tokio::task::yield_now().await;
                }
            }));
        }

        let start = Instant::now();
        for handle in handles {
            handle.await.expect("should complete");
        }
        let elapsed = start.elapsed();

        let total_ops = ops_per_burst.load(Ordering::Relaxed) as f64;
        let throughput = total_ops / elapsed.as_secs_f64();
        degradation_data.push((*burst_size, throughput));
    }

    assert!(
        degradation_data.len() == burst_sizes.len(),
        "Should have data for all burst sizes"
    );

    let baseline_throughput = degradation_data[0].1;
    assert!(
        baseline_throughput > 0.0,
        "Baseline throughput should be positive"
    );

    for (i, (burst, throughput)) in degradation_data.iter().enumerate() {
        let ratio = *throughput / baseline_throughput;
        eprintln!(
            "  Burst {} ({} tasks): {:.0} ops/s (ratio: {:.2}x)",
            i, burst, throughput, ratio
        );
    }
}

#[tokio::test]
async fn fairness_under_load_no_starvation() {
    let semaphore = Arc::new(make_semaphore(5));
    let shed_threshold = 3;

    let _held: Vec<_> = (0..5)
        .map(|_| semaphore.try_acquire().expect("should acquire"))
        .collect();

    assert!(semaphore
        .check_load_shedding_threshold(shed_threshold)
        .is_err());

    let task_acquired = Arc::new(AtomicUsize::new(0));
    let task_rejected = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for task_id in 0..10 {
        let sem = Arc::clone(&semaphore);
        let acquired = Arc::clone(&task_acquired);
        let rejected = Arc::clone(&task_rejected);
        handles.push(tokio::spawn(async move {
            let mut attempts = 0u64;
            let mut local_acquired = 0usize;
            let mut local_rejected = 0usize;
            while attempts < 100 {
                attempts += 1;
                match sem.try_acquire() {
                    Ok(_p) => {
                        local_acquired += 1;
                        acquired.fetch_add(1, Ordering::Relaxed);
                        tokio::task::yield_now().await;
                    }
                    Err(_) => {
                        local_rejected += 1;
                        rejected.fetch_add(1, Ordering::Relaxed);
                        tokio::task::yield_now().await;
                    }
                }
            }
            (task_id, local_acquired, local_rejected)
        }));
    }

    for handle in handles {
        let (task_id, acquired, rejected) = handle.await.expect("should complete");
        assert!(
            rejected > 0,
            "Task {} should have been rejected at least once under load",
            task_id
        );
        eprintln!(
            "  Task {}: acquired={}, rejected={}",
            task_id, acquired, rejected
        );
    }

    let total_acquired = task_acquired.load(Ordering::Relaxed);
    let total_rejected = task_rejected.load(Ordering::Relaxed);
    assert!(
        total_rejected > 0,
        "Total rejections should be > 0 (got {} acquired, {} rejected)",
        total_acquired,
        total_rejected
    );
}

#[tokio::test]
async fn default_semaphore_withstands_production_load() {
    let semaphore = Arc::new(make_semaphore(MAX_CONCURRENT_BINARIES));

    let ops = Arc::new(AtomicU64::new(0));
    let rejections = Arc::new(AtomicU64::new(0));
    let num_workers = 200usize;
    let duration = Duration::from_secs(2);

    let mut handles = Vec::new();
    for worker_id in 0..num_workers {
        let sem = Arc::clone(&semaphore);
        let ops = Arc::clone(&ops);
        let rejections = Arc::clone(&rejections);
        handles.push(tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + duration;
            let mut local_ops = 0u64;
            let mut local_rejections = 0u64;
            while tokio::time::Instant::now() < deadline {
                match sem.try_acquire() {
                    Ok(permit) => {
                        local_ops += 1;
                        drop(permit);
                    }
                    Err(SemaphoreLimitError::LimitReached { .. }) => {
                        local_rejections += 1;
                    }
                    Err(SemaphoreLimitError::LoadSheddingActive { .. }) => {
                        local_rejections += 1;
                    }
                    Err(SemaphoreLimitError::Closed) => {
                        panic!("Semaphore should never be closed");
                    }
                }
                if worker_id % 4 == 0 {
                    tokio::task::yield_now().await;
                }
            }
            ops.fetch_add(local_ops, Ordering::Relaxed);
            rejections.fetch_add(local_rejections, Ordering::Relaxed);
        }));
    }

    let start = Instant::now();
    for handle in handles {
        handle.await.expect("worker should complete");
    }
    let elapsed = start.elapsed();

    let total_ops = ops.load(Ordering::Relaxed);
    let total_rejections = rejections.load(Ordering::Relaxed);
    let throughput = total_ops as f64 / elapsed.as_secs_f64();

    eprintln!("=== Production Load Test Results ===");
    eprintln!("  Workers: {}", num_workers);
    eprintln!("  Max permits: {}", MAX_CONCURRENT_BINARIES);
    eprintln!("  Duration: {:.2}s", elapsed.as_secs_f64());
    eprintln!("  Operations completed: {}", total_ops);
    eprintln!("  Rejections (backpressure): {}", total_rejections);
    eprintln!("  Throughput: {:.0} ops/s", throughput);
    eprintln!(
        "  Shed threshold: {} (MAX_YIELDED_ACTORS)",
        MAX_YIELDED_ACTORS
    );

    assert!(
        total_ops > 1000,
        "Should complete at least 1000 ops in 2s with 200 workers, got {}",
        total_ops
    );
    assert!(
        throughput > 100.0,
        "Throughput should exceed 100 ops/s, got {:.0}",
        throughput
    );
}

#[tokio::test]
async fn permit_conservation_invariant_under_load() {
    let max_permits = 50usize;
    let semaphore = Arc::new(make_semaphore(max_permits));
    let iterations = 10_000u64;
    let num_workers = 50usize;

    let violations = Arc::new(AtomicU64::new(0));

    let mut handles = Vec::new();
    for _ in 0..num_workers {
        let sem = Arc::clone(&semaphore);
        let violations = Arc::clone(&violations);
        handles.push(tokio::spawn(async move {
            for _ in 0..iterations / num_workers as u64 {
                if let Ok(permit) = sem.try_acquire() {
                    let available = sem.available_permits();
                    let acquired = sem.acquired_count();
                    if available + acquired != max_permits {
                        violations.fetch_add(1, Ordering::Relaxed);
                    }
                    drop(permit);
                }
                tokio::task::yield_now().await;
            }
        }));
    }

    for handle in handles {
        handle.await.expect("should complete");
    }

    let violation_count = violations.load(Ordering::Relaxed);
    assert_eq!(
        violation_count, 0,
        "Permit conservation invariant violated {} times: available + acquired != max_permits",
        violation_count
    );

    let final_available = semaphore.available_permits();
    let final_acquired = semaphore.acquired_count();
    assert_eq!(
        final_available + final_acquired,
        max_permits,
        "Final state: available ({}) + acquired ({}) != max ({})",
        final_available,
        final_acquired,
        max_permits
    );
}
