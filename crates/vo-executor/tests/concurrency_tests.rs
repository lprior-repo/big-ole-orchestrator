//! Concurrency tests for vo-executor
//!
//! Tests concurrent workflow execution and resource management including:
//! - Semaphore-based concurrency limiting under stress
//! - Memory leak detection under sustained concurrent load
//! - Scheduler behavior under concurrent job scheduling
//! - Permit acquisition/release under concurrent load
//! - DashMap-based global state concurrent access stress testing

#[cfg(test)]
mod concurrency_resource_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use std::time::Duration;
    use vo_executor::scheduler::Scheduler;
    use vo_executor::{get_state_count, reset_all_state, Job, JobId, Schedule, SchedulerConfig};

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    // =========================================================================
    // Section 1: Semaphore Concurrency Limit Tests
    // =========================================================================

    #[tokio::test]
    async fn semaphore_concurrent_acquire_allows_up_to_limit() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 5,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let mut permits = Vec::new();
        for _ in 0..5 {
            if let Some(permit) = scheduler.try_acquire() {
                permits.push(permit);
            }
        }

        assert_eq!(permits.len(), 5, "Should acquire 5 permits at limit");
        assert!(
            scheduler.try_acquire().is_none(),
            "Sixth acquire should fail at limit"
        );
    }

    #[tokio::test]
    async fn semaphore_concurrent_acquire_strict_enforcement() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let permit1 = scheduler.try_acquire();
        let permit2 = scheduler.try_acquire();

        assert!(permit1.is_some());
        assert!(permit2.is_none());
    }

    #[tokio::test]
    async fn semaphore_zero_max_concurrent_allows_nothing() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 0,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        assert!(
            scheduler.try_acquire().is_none(),
            "Zero max_concurrent should allow no permits"
        );
    }

    #[tokio::test]
    async fn semaphore_high_concurrent_limit() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 100,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let mut permits = Vec::new();
        for _ in 0..100 {
            if let Some(permit) = scheduler.try_acquire() {
                permits.push(permit);
            }
        }

        assert_eq!(permits.len(), 100, "Should acquire all 100 permits");
        assert!(
            scheduler.try_acquire().is_none(),
            "Should not be able to acquire more than limit"
        );
    }

    // =========================================================================
    // Section 2: Memory Leak Detection Under Concurrent Load
    // =========================================================================

    #[tokio::test]
    async fn memory_leak_detection_state_count_after_sequential_executions() {
        let _guard = state_guard();
        use vo_executor::{execute_step, StepId};

        let initial_count = get_state_count();

        for i in 0..100 {
            let step_id = StepId::new(format!("leak-step-{}", i));
            let _ = execute_step(step_id, 5000).await;
        }

        let final_count = get_state_count();
        assert_eq!(
            final_count,
            initial_count + 100,
            "State count should grow by number of executions: initial={}, final={}",
            initial_count,
            final_count
        );
    }

    #[tokio::test]
    async fn memory_leak_detection_state_count_after_concurrent_executions() {
        let _guard = state_guard();
        use vo_executor::{execute_step, StepId};

        let initial_count = get_state_count();

        let mut handles = Vec::new();
        for i in 0..50 {
            let step_id = StepId::new(format!("concurrent-leak-{}", i));
            handles.push(tokio::spawn(
                async move { execute_step(step_id, 5000).await },
            ));
        }

        for handle in handles {
            let _ = handle.await;
        }

        let final_count = get_state_count();
        assert_eq!(
            final_count,
            initial_count + 50,
            "State count should grow by number of concurrent executions: initial={}, final={}",
            initial_count,
            final_count
        );
    }

    #[tokio::test]
    async fn memory_leak_detection_sustained_load() {
        let _guard = state_guard();
        use vo_executor::{execute_step, StepId};

        let initial_count = get_state_count();

        for round in 0..5 {
            let mut handles = Vec::new();
            for i in 0..20 {
                let step_id = StepId::new(format!("sustained-{}-{}", round, i));
                handles.push(tokio::spawn(
                    async move { execute_step(step_id, 5000).await },
                ));
            }

            for handle in handles {
                let _ = handle.await;
            }
        }

        let final_count = get_state_count();
        assert_eq!(
            final_count,
            initial_count + 100,
            "State count should grow by total executions across rounds: initial={}, final={}",
            initial_count,
            final_count
        );
    }

    // =========================================================================
    // Section 3: Concurrent Job Scheduling Tests
    // =========================================================================

    #[tokio::test]
    async fn scheduler_concurrent_scheduling_many_jobs() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        for i in 0..100 {
            let job = Job::new(
                JobId::new(i),
                format!("job-{}", i),
                Schedule::one_shot(Duration::from_millis(10)),
            );
            scheduler.schedule(job).expect("Schedule should succeed");
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due.len(), 100, "All 100 jobs should be due");
    }

    #[tokio::test]
    async fn scheduler_concurrent_poll_returns_due_jobs() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 50,
        };
        let mut scheduler = Scheduler::new(config);

        for i in 0..100 {
            let job = Job::new(
                JobId::new(i),
                format!("job-{}", i),
                Schedule::one_shot(Duration::from_millis(10)),
            );
            scheduler.schedule(job).unwrap();
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due.len(), 50, "Should return max_jobs_per_scan=50 due jobs");
    }

    #[tokio::test]
    async fn scheduler_concurrent_multiple_polls_exhaust_queue() {
        let _guard = state_guard();
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 10,
        };
        let mut scheduler = Scheduler::new(config);

        for i in 0..30 {
            let job = Job::new(
                JobId::new(i),
                format!("job-{}", i),
                Schedule::one_shot(Duration::from_millis(10)),
            );
            scheduler.schedule(job).unwrap();
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let batch1 = scheduler.poll_due_jobs(now_ms + 100);
        let batch2 = scheduler.poll_due_jobs(now_ms + 100);
        let batch3 = scheduler.poll_due_jobs(now_ms + 100);

        assert_eq!(batch1.len(), 10);
        assert_eq!(batch2.len(), 10);
        assert_eq!(batch3.len(), 10);
        assert_eq!(scheduler.len(), 0, "All jobs should be polled");
    }

    // =========================================================================
    // Section 4: Concurrent Execution Stress Tests
    // =========================================================================

    #[tokio::test]
    async fn stress_concurrent_tokio_join_many_tasks() {
        let _guard = state_guard();
        use vo_executor::{execute_step, StepId};

        let results = tokio::join!(
            execute_step(StepId::new("step-1".to_string()), 5000),
            execute_step(StepId::new("step-2".to_string()), 5000),
            execute_step(StepId::new("step-3".to_string()), 5000),
            execute_step(StepId::new("step-4".to_string()), 5000),
            execute_step(StepId::new("step-5".to_string()), 5000),
            execute_step(StepId::new("step-6".to_string()), 5000),
            execute_step(StepId::new("step-7".to_string()), 5000),
            execute_step(StepId::new("step-8".to_string()), 5000),
        );

        assert!(results.0.is_ok());
        assert!(results.1.is_ok());
        assert!(results.2.is_ok());
        assert!(results.3.is_ok());
        assert!(results.4.is_ok());
        assert!(results.5.is_ok());
        assert!(results.6.is_ok());
        assert!(results.7.is_ok());
    }

    #[tokio::test]
    async fn stress_concurrent_spawn_many_tasks() {
        let _guard = state_guard();
        use vo_executor::{execute_step, StepId};

        let mut handles = Vec::new();
        for i in 0..20 {
            let step_id = StepId::new(format!("step-{}", i % 5 + 1));
            handles.push(tokio::spawn(
                async move { execute_step(step_id, 5000).await },
            ));
        }

        let mut success_count = 0;
        for handle in handles {
            if handle.await.expect("Task should complete").is_ok() {
                success_count += 1;
            }
        }

        assert_eq!(success_count, 20);
    }

    #[tokio::test]
    async fn stress_concurrent_mixed_step_types() {
        let _guard = state_guard();
        use vo_executor::{execute_step, StepId};

        let mut handles = Vec::new();

        for _i in 0..10 {
            handles.push(tokio::spawn(async move {
                execute_step(StepId::new("step-1".to_string()), 5000).await
            }));
        }
        for _i in 0..5 {
            handles.push(tokio::spawn(async move {
                execute_step(StepId::new("step-fail".to_string()), 5000).await
            }));
        }
        for _i in 0..5 {
            handles.push(tokio::spawn(async move {
                execute_step(StepId::new("step-good".to_string()), 5000).await
            }));
        }

        let mut ok_count = 0;
        let mut fail_count = 0;
        for handle in handles {
            let result = handle.await.expect("Task should complete");
            if result.expect("Step execution failed").is_success() {
                ok_count += 1;
            } else {
                fail_count += 1;
            }
        }

        assert_eq!(ok_count, 15, "10 step-1 + 5 step-good should succeed");
        assert_eq!(fail_count, 5, "5 step-fail should return Failure result");
    }

    // =========================================================================
    // Section 5: Scheduler Start/Stop Lifecycle
    // =========================================================================

    #[tokio::test]
    async fn scheduler_lifecycle_start_stop_multiple_times() {
        let _guard = state_guard();
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        scheduler.start();
        assert!(scheduler.is_running());

        scheduler.stop();
        assert!(!scheduler.is_running());

        scheduler.start();
        assert!(scheduler.is_running());

        scheduler.stop();
        assert!(!scheduler.is_running());
    }

    #[tokio::test]
    async fn scheduler_cancel_after_start_stop() {
        let _guard = state_guard();
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();

        scheduler.start();
        scheduler.stop();

        let removed = scheduler.cancel(JobId::new(1));
        assert!(removed.is_some());
    }

    // =========================================================================
    // Section 6: Concurrency Atomic Counter Tests
    // =========================================================================

    #[tokio::test]
    async fn atomic_concurrent_counter_increment() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let mut handles = Vec::new();
        for _ in 0..100 {
            handles.push(tokio::spawn(async move {
                COUNTER.fetch_add(1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }

        assert_eq!(COUNTER.load(Ordering::SeqCst), 100);
    }

    #[tokio::test]
    async fn atomic_concurrent_counter_increment_under_mutex() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        static MUTEX: Mutex<()> = Mutex::new(());

        let _guard = state_guard();

        let mut handles = Vec::new();
        for _ in 0..100 {
            handles.push(tokio::spawn(async move {
                let _lock = MUTEX.lock().unwrap();
                COUNTER.fetch_add(1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }

        assert_eq!(COUNTER.load(Ordering::SeqCst), 100);
    }

    // =========================================================================
    // Section 7: Error State Under Concurrent Load
    // =========================================================================

    #[tokio::test]
    async fn concurrent_transient_errors_all_recorded() {
        let _guard = state_guard();
        use vo_executor::{execute_step, get_last_error, StepId};

        let mut handles = Vec::new();
        for i in 0..10 {
            let step_id = StepId::new(format!("transient-step-{}", i));
            handles.push(tokio::spawn(async move {
                execute_step(step_id.clone(), 5000).await
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        for i in 0..10 {
            let step_id = StepId::new(format!("transient-step-{}", i));
            assert!(
                get_last_error(&step_id).is_some(),
                "Error should be recorded for transient-step-{}",
                i
            );
        }
    }

    #[tokio::test]
    async fn concurrent_unknown_steps_all_return_not_found() {
        let _guard = state_guard();
        use vo_executor::{execute_step, StepId};

        let mut handles = Vec::new();
        for i in 0..10 {
            let step_id = StepId::new(format!("unknown-concurrent-{}", i));
            handles.push(tokio::spawn(
                async move { execute_step(step_id, 5000).await },
            ));
        }

        let mut error_count = 0;
        for handle in handles {
            if let Ok(Err(_)) = handle.await {
                error_count += 1;
            }
        }

        assert_eq!(error_count, 10, "All unknown steps should return errors");
    }

    // =========================================================================
    // Section 8: DashMap Global State Concurrent Access Stress Tests
    // =========================================================================

    #[tokio::test]
    async fn stress_concurrent_set_get_state_no_data_loss() {
        let _guard = state_guard();
        use vo_executor::state::{get_state, set_state, StepState};

        const THREADS: usize = 10;
        const ITERATIONS: usize = 1000;

        let mut handles = Vec::new();

        for t in 0..THREADS {
            let handle = tokio::spawn(async move {
                let base = t * ITERATIONS;
                for i in 0..ITERATIONS {
                    let step_id = format!("stress-set-get-{}", base + i);
                    let value = (base + i) as u64;

                    set_state(
                        &step_id,
                        StepState::Completed {
                            output: value.to_string(),
                        },
                    );

                    let retrieved = get_state(&step_id);
                    assert!(
                        matches!(&retrieved, StepState::Completed { output } if output == &value.to_string()),
                        "Data loss detected for key {}: expected value {}, got {:?}",
                        step_id,
                        value,
                        retrieved
                    );
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }
    }

    #[tokio::test]
    async fn stress_concurrent_shared_key_set_get_no_data_loss() {
        let _guard = state_guard();
        use vo_executor::state::{get_state, set_state, StepState};

        const THREADS: usize = 10;
        const ITERATIONS: usize = 500;
        const SHARED_KEY: &str = "shared-stress-key";

        let mut handles = Vec::new();

        for t in 0..THREADS {
            let handle = tokio::spawn(async move {
                let expected_value = (t + 1) as u64;
                for _i in 0..ITERATIONS {
                    set_state(
                        SHARED_KEY,
                        StepState::Completed {
                            output: expected_value.to_string(),
                        },
                    );

                    let retrieved = get_state(SHARED_KEY);
                    if let StepState::Completed { output, .. } = &retrieved {
                        assert_eq!(
                            output, &expected_value.to_string(),
                            "Got unexpected value for shared key: expected {}, got {}",
                            expected_value, output
                        );
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }
    }

    #[tokio::test]
    async fn stress_concurrent_set_error_clear_error_last_write_wins() {
        let _guard = state_guard();
        use vo_executor::state::{get_last_error, set_error};
        use vo_executor::errors::ExecuteNodeError;
        use std::sync::atomic::{AtomicU64, Ordering};

        static LAST_WRITER_ID: AtomicU64 = AtomicU64::new(0);

        const THREADS: usize = 10;
        const ITERATIONS: usize = 200;
        const SHARED_KEY: &str = "error-stress-key";

        let mut handles = Vec::new();

        for t in 0..THREADS {
            let handle = tokio::spawn(async move {
                let writer_id = (t + 1) as u64;
                for _i in 0..ITERATIONS {
                    let err = ExecuteNodeError::ExecutionCancelled {
                        reason: format!("writer-{}", writer_id),
                    };
                    set_error(SHARED_KEY, err);
                    LAST_WRITER_ID.store(writer_id, Ordering::SeqCst);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }

        let final_writer = LAST_WRITER_ID.load(Ordering::SeqCst);
        let retrieved = get_last_error(SHARED_KEY);
        assert!(
            retrieved.is_some(),
            "Error should exist for shared key"
        );
        if let Some(err) = retrieved {
            match err {
                ExecuteNodeError::ExecutionCancelled { reason } => {
                    assert!(
                        reason.ends_with(&format!("writer-{}", final_writer)),
                        "Expected last-write-wins error from writer-{}, got reason: {}",
                        final_writer,
                        reason
                    );
                }
                _ => panic!("Expected ExecutionCancelled error, got {:?}", err),
            }
        }
    }

    #[tokio::test]
    async fn stress_concurrent_different_keys_error_count_monotonic() {
        let _guard = state_guard();
        use vo_executor::{get_error_count, set_error};
        use vo_executor::errors::ExecuteNodeError;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static MAX_COUNT_SEEN: AtomicUsize = AtomicUsize::new(0);

        const THREADS: usize = 8;
        const KEYS_PER_THREAD: usize = 50;

        let mut handles = Vec::new();

        for t in 0..THREADS {
            let handle = tokio::spawn(async move {
                let base = t * KEYS_PER_THREAD;
                for i in 0..KEYS_PER_THREAD {
                    let step_id = format!("err-count-stress-{}-{}", t, i);
                    let err = ExecuteNodeError::TimeoutExceeded {
                        elapsed_ms: base as u64 + i as u64,
                        limit_ms: 1000,
                    };
                    set_error(&step_id, err);

                    let current_count = get_error_count();
                    let max_seen = MAX_COUNT_SEEN.load(Ordering::SeqCst);
                    if current_count > max_seen {
                        MAX_COUNT_SEEN.store(current_count, Ordering::SeqCst);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }

        let max_seen = MAX_COUNT_SEEN.load(Ordering::SeqCst);
        let final_count = get_error_count();
        assert_eq!(
            max_seen, final_count,
            "Max count seen ({}) should equal final count ({}) - monotonicity violation",
            max_seen, final_count
        );
        assert_eq!(
            final_count,
            THREADS * KEYS_PER_THREAD,
            "Final error count should equal threads * keys_per_thread"
        );
    }

    #[tokio::test]
    async fn stress_concurrent_set_clear_state_count_monotonic() {
        let _guard = state_guard();
        use vo_executor::state::{set_state, StepState};
        use vo_executor::get_state_count;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static MAX_COUNT_SEEN: AtomicUsize = AtomicUsize::new(0);

        const THREADS: usize = 8;
        const KEYS_PER_THREAD: usize = 50;

        let mut handles = Vec::new();

        for t in 0..THREADS {
            let handle = tokio::spawn(async move {
                let base = t * KEYS_PER_THREAD;
                for i in 0..KEYS_PER_THREAD {
                    let step_id = format!("state-count-stress-{}-{}", t, i);
                    set_state(
                        &step_id,
                        StepState::Completed {
                            output: format!("{}-{}", base, i),
                        },
                    );

                    let current_count = get_state_count();
                    let max_seen = MAX_COUNT_SEEN.load(Ordering::SeqCst);
                    if current_count > max_seen {
                        MAX_COUNT_SEEN.store(current_count, Ordering::SeqCst);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }

        let max_seen = MAX_COUNT_SEEN.load(Ordering::SeqCst);
        let final_count = get_state_count();
        assert_eq!(
            max_seen, final_count,
            "Max count seen ({}) should equal final count ({}) - monotonicity violation",
            max_seen, final_count
        );
        assert_eq!(
            final_count,
            THREADS * KEYS_PER_THREAD,
            "Final state count should equal threads * keys_per_thread"
        );
    }

    #[tokio::test]
    async fn stress_reset_all_state_during_active_operations_no_panic() {
        let _guard = state_guard();
        use vo_executor::state::{get_state, set_state, StepState};
        use vo_executor::reset_all_state;

        const OUTER_ITERS: usize = 20;
        const INNER_OPS: usize = 100;

        for outer in 0..OUTER_ITERS {
            let mut handles = Vec::new();

            let writer_handle = tokio::spawn(async move {
                for i in 0..INNER_OPS {
                    let step_id = format!("reset-stress-writer-{}-{}", outer, i);
                    set_state(
                        &step_id,
                        StepState::Completed {
                            output: format!("data-{}", i),
                        },
                    );
                }
            });
            handles.push(writer_handle);

            let reader_handle = tokio::spawn(async move {
                for _i in 0..INNER_OPS {
                    let _state = get_state("reset-stress-reader-check");
                }
            });
            handles.push(reader_handle);

            let clearer_handle = tokio::spawn(async move {
                for i in 0..INNER_OPS {
                    let step_id = format!("reset-stress-clear-{}-{}", outer, i);
                    set_state(
                        &step_id,
                        StepState::Cancelled {
                            reason: format!("cleared-{}", i),
                        },
                    );
                }
            });
            handles.push(clearer_handle);

            for handle in handles {
                handle.await.expect("Task should complete");
            }

            reset_all_state();

            let count = vo_executor::get_state_count();
            let err_count = vo_executor::get_error_count();
            assert_eq!(
                count, 0,
                "After reset_all_state, count should be 0, got {} at outer iter {}",
                count, outer
            );
            assert_eq!(
                err_count, 0,
                "After reset_all_state, error_count should be 0, got {} at outer iter {}",
                err_count, outer
            );
        }
    }

    #[tokio::test]
    async fn stress_concurrent_mixed_state_and_error_operations() {
        let _guard = state_guard();
        use vo_executor::state::{get_state, set_state, StepState, clear_error, set_error};
        use vo_executor::errors::ExecuteNodeError;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static TOTAL_OPS: AtomicUsize = AtomicUsize::new(0);

        const THREADS: usize = 6;
        const OPS_PER_THREAD: usize = 300;

        let mut handles = Vec::new();

        for t in 0..THREADS {
            let handle = tokio::spawn(async move {
                for i in 0..OPS_PER_THREAD {
                    let op_type = (i % 4) as u8;
                    let step_id = format!("mixed-stress-{}-{}", t, i);

                    match op_type {
                        0 => {
                            set_state(
                                &step_id,
                                StepState::Completed {
                                    output: format!("out-{}", i),
                                },
                            );
                        }
                        1 => {
                            let _ = get_state(&step_id);
                        }
                        2 => {
                            let err = ExecuteNodeError::ExecutionCancelled {
                                reason: format!("err-{}-{}", t, i),
                            };
                            set_error(&step_id, err);
                        }
                        3 => {
                            clear_error(&step_id);
                        }
                        _ => unreachable!(),
                    }

                    TOTAL_OPS.fetch_add(1, Ordering::SeqCst);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task should complete");
        }

        let total = TOTAL_OPS.load(Ordering::SeqCst);
        assert_eq!(
            total,
            THREADS * OPS_PER_THREAD,
            "All operations should be counted"
        );
    }

    #[tokio::test]
    async fn stress_high_contention_same_key_many_writers() {
        let _guard = state_guard();
        use vo_executor::state::{get_state, set_state, StepState};

        const WRITERS: usize = 20;
        const ITERATIONS: usize = 500;
        const SHARED_KEY: &str = "high-contention-key";

        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(WRITERS + 1));
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mut handles = Vec::new();

        for w in 0..WRITERS {
            let barrier = barrier.clone();
            let counter = counter.clone();
            let handle = tokio::spawn(async move {
                barrier.wait().await;
                let writer_id = w as u64;
                for _i in 0..ITERATIONS {
                    set_state(
                        SHARED_KEY,
                        StepState::Completed {
                            output: writer_id.to_string(),
                        },
                    );
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            });
            handles.push(handle);
        }

        barrier.wait().await;

        for handle in handles {
            handle.await.expect("Task should complete");
        }

        let total_writes = counter.load(Ordering::SeqCst);
        assert_eq!(
            total_writes,
            WRITERS * ITERATIONS,
            "All {} writers * {} iterations = {} writes should complete",
            WRITERS,
            ITERATIONS,
            WRITERS * ITERATIONS
        );

        let final_state = get_state(SHARED_KEY);
        assert!(
            matches!(&final_state, StepState::Completed { .. }),
            "Shared key should have a valid state after all writes, got {:?}",
            final_state
        );
    }
}
