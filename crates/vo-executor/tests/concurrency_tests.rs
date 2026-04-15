//! Concurrency tests for vo-executor
//!
//! Tests concurrent workflow execution and resource management including:
//! - Semaphore-based concurrency limiting under stress
//! - Memory leak detection under sustained concurrent load
//! - Scheduler behavior under concurrent job scheduling
//! - Permit acquisition/release under concurrent load

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
}
