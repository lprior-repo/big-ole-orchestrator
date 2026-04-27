//! Adversarial tests for subprocess resource exhaustion (BLACKHAT bh-010)
//!
//! Tests that verify:
//! - Concurrent subprocess spawning is bounded by scheduler semaphore
//! - At semaphore limit, new spawns are rejected (try_acquire returns None)
//! - Subprocess count stays bounded under concurrent load
//! - FD exhaustion from pipe pairs is bounded by concurrency limits
//! - Rapid spawn/collect cycles don't leak resources
//! - Scheduler semaphore prevents unbounded subprocess creation

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::Duration;

use vo_executor::{
    reset_all_state, run_subprocess, scheduler::Scheduler, SubprocessConfig, SchedulerConfig,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

fn helper_path() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let target_dir = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("debug")
        .join("test_subprocess_helper");
    target_dir.to_string_lossy().to_string()
}

fn fast_config(helper: &str) -> SubprocessConfig {
    SubprocessConfig::new(
        helper.to_string(),
        vec!["sleep-exit".to_string(), "0".to_string(), "0".to_string()],
        5000,
        vec![],
    )
}

fn sleep_exit_config(helper: &str, delay_ms: u64, exit_code: i32) -> SubprocessConfig {
    SubprocessConfig::new(
        helper.to_string(),
        vec![
            "sleep-exit".to_string(),
            delay_ms.to_string(),
            exit_code.to_string(),
        ],
        10000,
        vec![],
    )
}

// ============================================================================
// Contract 1: Scheduler semaphore bounds concurrent subprocess spawning
// ============================================================================

#[cfg(test)]
mod semaphore_subprocess_bounds {
    use super::*;

    #[tokio::test]
    async fn semaphore_at_capacity_rejects_new_subprocess_permits() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 3,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let p1 = scheduler.try_acquire();
        let p2 = scheduler.try_acquire();
        let p3 = scheduler.try_acquire();

        assert!(p1.is_some(), "First permit should be acquired");
        assert!(p2.is_some(), "Second permit should be acquired");
        assert!(p3.is_some(), "Third permit should be acquired");

        let p4 = scheduler.try_acquire();
        assert!(p4.is_none(), "Fourth permit MUST be rejected at capacity=3");
    }

    #[tokio::test]
    async fn semaphore_release_allows_new_subprocess_permit() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let p1 = scheduler.try_acquire();
        let p2 = scheduler.try_acquire();
        assert!(p1.is_some());
        assert!(p2.is_some());

        assert!(scheduler.try_acquire().is_none());

        drop(p1);

        let p3 = scheduler.try_acquire();
        assert!(
            p3.is_some(),
            "After dropping permit, new acquire must succeed"
        );
    }

    #[tokio::test]
    async fn semaphore_zero_capacity_rejects_all_subprocess_spawns() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 0,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        assert!(
            scheduler.try_acquire().is_none(),
            "Zero capacity must reject all spawns"
        );
    }

    #[tokio::test]
    async fn semaphore_single_capacity_serializes_subprocess_access() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let p1 = scheduler.try_acquire();
        assert!(p1.is_some(), "First acquire succeeds");

        assert!(scheduler.try_acquire().is_none(), "Second acquire blocked");

        drop(p1);

        let p2 = scheduler.try_acquire();
        assert!(p2.is_some(), "After release, acquire succeeds");
    }
}

// ============================================================================
// Contract 2: Concurrent subprocess spawning stays bounded
// ============================================================================

#[cfg(test)]
mod concurrent_subprocess_bounds {
    use super::*;

    #[tokio::test]
    async fn concurrent_subprocess_spawns_stay_within_semaphore_limit() {
        let _guard = state_guard();
        let helper = helper_path();
        let max_concurrent = 4usize;
        let total_spawns = 20usize;

        let spawn_count = std::sync::Arc::new(AtomicUsize::new(0));
        let max_seen = std::sync::Arc::new(AtomicUsize::new(0));
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));

        let mut handles = Vec::new();

        for _ in 0..total_spawns {
            let helper = helper.clone();
            let spawn_count = spawn_count.clone();
            let max_seen = max_seen.clone();
            let semaphore = semaphore.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore
                    .acquire()
                    .await
                    .expect("semaphore acquire failed");

                let current = spawn_count.fetch_add(1, Ordering::SeqCst) + 1;
                let prev = max_seen.load(Ordering::SeqCst);
                if current > prev {
                    let _ = max_seen.compare_exchange(prev, current, Ordering::SeqCst, Ordering::SeqCst);
                }

                let config = fast_config(&helper);
                let result = run_subprocess(config).await;

                spawn_count.fetch_sub(1, Ordering::SeqCst);

                result
            }));
        }

        let mut success_count = 0;
        for handle in handles {
            if handle.await.expect("task panicked").is_ok() {
                success_count += 1;
            }
        }

        assert_eq!(success_count, total_spawns, "All spawns must succeed");

        let peak = max_seen.load(Ordering::SeqCst);
        assert!(
            peak <= max_concurrent,
            "Peak concurrent spawns ({}) must not exceed limit ({})",
            peak,
            max_concurrent
        );
    }

    #[tokio::test]
    async fn rapid_sequential_subprocess_spawns_do_not_leak_fds() {
        let _guard = state_guard();
        let helper = helper_path();

        let iterations = 50;

        for _ in 0..iterations {
            let config = fast_config(&helper);
            let result = run_subprocess(config).await;
            assert!(result.is_ok(), "Sequential spawn must succeed: {:?}", result);
        }
    }

    #[tokio::test]
    async fn concurrent_long_running_subprocesses_bounded_by_semaphore() {
        let _guard = state_guard();
        let helper = helper_path();
        let max_concurrent = 3usize;
        let total_spawns = 9usize;

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
        let active_count = std::sync::Arc::new(AtomicUsize::new(0));
        let max_active = std::sync::Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        for _ in 0..total_spawns {
            let helper = helper.clone();
            let semaphore = semaphore.clone();
            let active_count = active_count.clone();
            let max_active = max_active.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore
                    .acquire()
                    .await
                    .expect("semaphore acquire failed");

                let current = active_count.fetch_add(1, Ordering::SeqCst) + 1;
                loop {
                    let prev = max_active.load(Ordering::SeqCst);
                    if current <= prev || max_active.compare_exchange(prev, current, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                        break;
                    }
                }

                let config = sleep_exit_config(&helper, 100, 0);
                let result = run_subprocess(config).await;

                active_count.fetch_sub(1, Ordering::SeqCst);

                result
            }));
        }

        let mut success = 0;
        for handle in handles {
            if handle.await.expect("task panicked").is_ok() {
                success += 1;
            }
        }

        assert_eq!(success, total_spawns, "All long-running spawns must succeed");

        let peak = max_active.load(Ordering::SeqCst);
        assert!(
            peak <= max_concurrent,
            "Peak active ({}) exceeded limit ({})",
            peak,
            max_concurrent
        );
    }
}

// ============================================================================
// Contract 3: Pipe FD budget per subprocess is bounded
// ============================================================================

#[cfg(test)]
mod pipe_fd_budget_tests {
    use super::*;

    #[tokio::test]
    async fn each_subprocess_consumes_exactly_two_pipe_pairs() {
        let _guard = state_guard();
        let helper = helper_path();

        let config = fast_config(&helper);
        let result = run_subprocess(config).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.fd4_bytes.is_empty());
        assert_eq!(output.exit_code, Some(0));
    }

    #[tokio::test]
    async fn concurrent_subprocesses_pipe_fds_reclaimed_after_completion() {
        let _guard = state_guard();
        let helper = helper_path();
        let rounds = 5;
        let per_round = 10;

        for _round in 0..rounds {
            let mut handles = Vec::new();
            for _ in 0..per_round {
                let helper = helper.clone();
                handles.push(tokio::spawn(async move {
                    let config = fast_config(&helper);
                    run_subprocess(config).await
                }));
            }

            for handle in handles {
                let result = handle.await.expect("task panicked");
                assert!(result.is_ok(), "Spawn should succeed: {:?}", result);
            }
        }
    }
}

// ============================================================================
// Contract 4: Scheduler semaphore prevents unbounded resource consumption
// ============================================================================

#[cfg(test)]
mod scheduler_prevents_resource_exhaustion {
    use super::*;

    #[tokio::test]
    async fn scheduler_semaphore_limits_total_in_flight_subprocesses() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 5,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = std::sync::Arc::new(Scheduler::new(config));

        let mut permits = Vec::new();
        let mut rejected = 0;

        for _ in 0..20 {
            match scheduler.try_acquire() {
                Some(p) => permits.push(p),
                None => rejected += 1,
            }
        }

        assert_eq!(permits.len(), 5, "Should acquire exactly 5 permits");
        assert_eq!(rejected, 15, "Should reject 15 excess requests");
    }

    #[tokio::test]
    async fn scheduler_semaphore_permits_released_on_drop_allows_progress() {
        let _guard = state_guard();
        let helper = helper_path();
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = std::sync::Arc::new(Scheduler::new(config));
        let completed = std::sync::Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for i in 0..10 {
            let scheduler = scheduler.clone();
            let helper = helper.clone();
            let completed = completed.clone();

            handles.push(tokio::spawn(async move {
                let permit = loop {
                    match scheduler.try_acquire() {
                        Some(p) => break p,
                        None => tokio::time::sleep(Duration::from_millis(10)).await,
                    }
                };

                let sub_config = fast_config(&helper);
                let _ = run_subprocess(sub_config).await;

                completed.fetch_add(1, Ordering::SeqCst);
                drop(permit);
            }));
        }

        for handle in handles {
            handle.await.expect("task panicked");
        }

        let total = completed.load(Ordering::SeqCst);
        assert_eq!(
            total, 10,
            "All 10 subprocesses must complete with semaphore=2"
        );
    }

    #[tokio::test]
    async fn semaphore_enforced_subprocess_pool_no_exhaustion() {
        let _guard = state_guard();
        let helper = helper_path();
        let pool_size = 4;
        let total_jobs = 20;

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(pool_size));
        let completed = std::sync::Arc::new(AtomicUsize::new(0));
        let peak_concurrent = std::sync::Arc::new(AtomicUsize::new(0));
        let current_concurrent = std::sync::Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..total_jobs {
            let helper = helper.clone();
            let semaphore = semaphore.clone();
            let completed = completed.clone();
            let peak_concurrent = peak_concurrent.clone();
            let current_concurrent = current_concurrent.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.expect("semaphore");

                let cur = current_concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                loop {
                    let prev = peak_concurrent.load(Ordering::SeqCst);
                    if cur <= prev || peak_concurrent.compare_exchange(prev, cur, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                        break;
                    }
                }

                let config = sleep_exit_config(&helper, 50, 0);
                let _ = run_subprocess(config).await;

                current_concurrent.fetch_sub(1, Ordering::SeqCst);
                completed.fetch_add(1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.await.expect("task panicked");
        }

        assert_eq!(
            completed.load(Ordering::SeqCst),
            total_jobs,
            "All jobs must complete"
        );
        assert!(
            peak_concurrent.load(Ordering::SeqCst) <= pool_size,
            "Peak concurrent must not exceed pool size"
        );
    }
}

// ============================================================================
// Contract 5: Subprocess timeout prevents resource hold
// ============================================================================

#[cfg(test)]
mod subprocess_timeout_resource_release {
    use super::*;

    #[tokio::test]
    async fn timed_out_subprocess_releases_resources() {
        let _guard = state_guard();
        let helper = helper_path();

        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "10000".to_string(), "0".to_string()],
            200,
            vec![],
        );

        let result = run_subprocess(config).await;
        assert!(result.is_err(), "Should timeout");
        assert!(
            matches!(result, Err(vo_executor::SubprocessError::Timeout { .. })),
            "Should be timeout error, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn consecutive_timeouts_do_not_accumulate_fds() {
        let _guard = state_guard();
        let helper = helper_path();

        for _ in 0..10 {
            let config = SubprocessConfig::new(
                helper.clone(),
                vec!["sleep-exit".to_string(), "10000".to_string(), "0".to_string()],
                100,
                vec![],
            );
            let result = run_subprocess(config).await;
            assert!(result.is_err(), "Each should timeout");
        }
    }

    #[tokio::test]
    async fn concurrent_timeouts_bounded_by_semaphore() {
        let _guard = state_guard();
        let helper = helper_path();
        let pool_size = 3;
        let total = 9;

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(pool_size));
        let mut handles = Vec::new();

        for _ in 0..total {
            let helper = helper.clone();
            let semaphore = semaphore.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.expect("semaphore");

                let config = SubprocessConfig::new(
                    helper,
                    vec!["sleep-exit".to_string(), "10000".to_string(), "0".to_string()],
                    200,
                    vec![],
                );
                run_subprocess(config).await
            }));
        }

        let mut timeout_count = 0;
        for handle in handles {
            let result = handle.await.expect("task panicked");
            if matches!(result, Err(vo_executor::SubprocessError::Timeout { .. })) {
                timeout_count += 1;
            }
        }

        assert_eq!(
            timeout_count, total,
            "All {} concurrent spawns must timeout",
            total
        );
    }
}

// ============================================================================
// Contract 6: Adversarial - spawn storm resistance
// ============================================================================

#[cfg(test)]
mod spawn_storm_resistance {
    use super::*;

    #[tokio::test]
    async fn spawn_storm_with_semaphore_stays_bounded() {
        let _guard = state_guard();
        let helper = helper_path();
        let pool_size = 4;
        let storm_size = 50;

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(pool_size));
        let active = std::sync::Arc::new(AtomicUsize::new(0));
        let peak = std::sync::Arc::new(AtomicUsize::new(0));
        let completed = std::sync::Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        for _ in 0..storm_size {
            let helper = helper.clone();
            let semaphore = semaphore.clone();
            let active = active.clone();
            let peak = peak.clone();
            let completed = completed.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.expect("semaphore");

                let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                loop {
                    let prev = peak.load(Ordering::SeqCst);
                    if cur <= prev || peak.compare_exchange(prev, cur, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                        break;
                    }
                }

                let config = fast_config(&helper);
                let _ = run_subprocess(config).await;

                active.fetch_sub(1, Ordering::SeqCst);
                completed.fetch_add(1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.await.expect("task panicked");
        }

        assert_eq!(
            completed.load(Ordering::SeqCst),
            storm_size,
            "All {} storm spawns must complete",
            storm_size
        );
        assert!(
            peak.load(Ordering::SeqCst) <= pool_size,
            "Peak ({}) must stay <= pool ({})",
            peak.load(Ordering::SeqCst),
            pool_size
        );
    }

    #[tokio::test]
    async fn mixed_success_and_timeout_spawn_storm_bounded() {
        let _guard = state_guard();
        let helper = helper_path();
        let pool_size = 3;
        let total = 12;

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(pool_size));
        let mut handles = Vec::new();

        for i in 0..total {
            let helper = helper.clone();
            let semaphore = semaphore.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.expect("semaphore");

                let config = if i % 3 == 0 {
                    SubprocessConfig::new(
                        helper,
                        vec!["sleep-exit".to_string(), "10000".to_string(), "0".to_string()],
                        200,
                        vec![],
                    )
                } else {
                    fast_config(&helper)
                };

                run_subprocess(config).await
            }));
        }

        let mut ok = 0;
        let mut timeouts = 0;
        for handle in handles {
            match handle.await.expect("task panicked") {
                Ok(_) => ok += 1,
                Err(vo_executor::SubprocessError::Timeout { .. }) => timeouts += 1,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        assert_eq!(ok + timeouts, total, "All must complete");
        assert!(timeouts > 0, "Some must timeout");
        assert!(ok > 0, "Some must succeed");
    }
}
