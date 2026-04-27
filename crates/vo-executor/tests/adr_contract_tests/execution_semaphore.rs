use super::common::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn semaphore_concurrent_limit_blocks_excess() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let p1 = scheduler.try_acquire();
        let p2 = scheduler.try_acquire();
        let p3 = scheduler.try_acquire();

        assert!(p1.is_some(), "First permit should be acquired");
        assert!(p2.is_some(), "Second permit should be acquired");
        assert!(p3.is_none(), "Third permit should be blocked (limit=2)");

        drop(p1);
        let p4 = scheduler.try_acquire();
        assert!(
            p4.is_some(),
            "After releasing one permit, new acquire should succeed"
        );
    }

    #[tokio::test]
    async fn semaphore_zero_concurrent_blocks_all() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 0,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let permit = scheduler.try_acquire();
        assert!(
            permit.is_none(),
            "Zero concurrency should block all permits"
        );
    }

    #[tokio::test]
    async fn semaphore_large_concurrent_all_acquired() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 100,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let mut permits = Vec::new();
        for i in 0..100 {
            let permit = scheduler.try_acquire();
            assert!(
                permit.is_some(),
                "Permit {} should be acquired (limit=100)",
                i
            );
            permits.push(permit);
        }

        let overflow = scheduler.try_acquire();
        assert!(overflow.is_none(), "101st permit should be blocked");
    }

    #[tokio::test]
    async fn semaphore_permit_release_allows_reacquire() {
        let _guard = state_guard();
        let config = vo_executor::SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = vo_executor::scheduler::Scheduler::new(config);

        let p1 = scheduler.try_acquire();
        assert!(p1.is_some());

        assert!(scheduler.try_acquire().is_none());

        drop(p1);

        let p2 = scheduler.try_acquire();
        assert!(p2.is_some(), "Should reacquire after release");
    }

    #[tokio::test]
    async fn backpressure_concurrent_steps_execute_independently() {
        let _guard = state_guard();
        let step_names = ["step-1", "step-good", "step-fail"];
        let step_ids: Vec<_> = (0..10)
            .map(|i| StepId::new(step_names[i % 3].to_string()))
            .collect();

        let handles: Vec<_> = step_ids
            .into_iter()
            .map(|sid| tokio::spawn(execute_step(sid, 5000)))
            .collect();

        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await.expect("task should complete");
            assert!(
                result.is_ok() || matches!(result, Err(ExecuteNodeError::TransientError { .. }))
            );
        }
    }

    #[tokio::test]
    async fn backpressure_burst_execution_all_succeed() {
        let _guard = state_guard();
        let handles: Vec<_> = (0..50)
            .map(|_| tokio::spawn(execute_step(StepId::new("step-good".to_string()), 5000)))
            .collect();

        let mut success_count = 0;
        for handle in handles {
            match handle.await.expect("task should complete") {
                Ok(StepResult::Success { .. }) => success_count += 1,
                other => panic!("Expected Success, got {:?}", other),
            }
        }
        assert_eq!(success_count, 50, "All 50 burst executions should succeed");
    }
}
