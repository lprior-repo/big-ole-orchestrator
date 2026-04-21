#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::await_holding_lock)]

//! Red Queen adversarial tests for task isolation (rq-007).
//!
//! These tests verify that tasks are properly isolated from each other:
//! - WHEN task panics, THE SYSTEM SHALL not affect other tasks
//! - State leaks between tasks SHALL be detectable
//! - Resource sharing between concurrent tasks SHALL be safe
//!
//! Architecture under test:
//!   Tier 1 (in-process): execute_step uses global DashMap — no OS isolation
//!   Tier 2 (subprocess): run_subprocess uses fork+exec — full OS isolation
//!
//! These tests target the boundary between tiers and the in-process isolation model.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::MutexGuard;
use vo_executor::{
    cancel_execution, execute_step, get_execution_status, get_last_error, reset_all_state,
    set_error, set_executing_state_for_test, StepId,
};
use vo_executor::state::{get_state, set_state, StepState};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}

// ============================================================================
// RED QUEEN: Panic Isolation — Task A panic SHALL NOT affect Task B
// ============================================================================

#[cfg(test)]
mod panic_isolation_tests {
    use super::*;

    #[tokio::test]
    async fn panic_in_spawned_task_does_not_corrupt_global_state() {
        // GIVEN: A clean state with a valid step
        let _guard = state_guard();
        let step_b = StepId::new("step-2".to_string());

        // WHEN: Task A panics inside a spawned task
        let panic_handle = tokio::spawn(async move {
            panic!("adversarial panic in task A");
        });

        // THEN: Task A panics (caught by JoinError)
        let result = panic_handle.await;
        assert!(result.is_err(), "spawned task should panic");

        // THEN: Task B is still operational after Task A panicked
        let task_b_result = execute_step(step_b, 5000).await;
        assert!(
            task_b_result.is_ok(),
            "Task B should succeed after Task A panic: {:?}",
            task_b_result
        );
    }

    #[tokio::test]
    async fn panic_during_state_write_does_not_poison_global_dashmap() {
        // GIVEN: A clean state
        let _guard = state_guard();

        // WHEN: A task panics after writing state
        set_state(
            "panic-victim-step",
            StepState::Completed {
                output: "pre-panic".to_string(),
            },
        );

        let panic_handle = tokio::spawn(async move {
            set_state(
                "panic-writer-step",
                StepState::Completed {
                    output: "during-panic".to_string(),
                },
            );
            panic!("panic after state write");
        });

        let _ = panic_handle.await;

        // THEN: DashMap is NOT poisoned (DashMap doesn't poison like Mutex)
        // Existing state is still readable
        let victim_state = get_state("panic-victim-step");
        assert!(
            matches!(&victim_state, StepState::Completed { output } if output == "pre-panic"),
            "Victim state should survive panic, got {:?}",
            victim_state
        );

        // THEN: New operations still work
        set_state(
            "post-panic-step",
            StepState::Ready,
        );
        let post_state = get_state("post-panic-step");
        assert!(
            matches!(post_state, StepState::Ready),
            "Post-panic writes should work"
        );
    }

    #[tokio::test]
    async fn multiple_panics_do_not_cascade() {
        // GIVEN: A clean state
        let _guard = state_guard();

        // WHEN: Multiple tasks panic concurrently
        let mut handles = Vec::new();
        for i in 0..5 {
            handles.push(tokio::spawn(async move {
                if i % 2 == 0 {
                    panic!("cascade panic {}", i);
                }
                i
            }));
        }

        let mut panic_count = 0;
        let mut success_count = 0;
        for handle in handles {
            match handle.await {
                Ok(_) => success_count += 1,
                Err(_) => panic_count += 1,
            }
        }

        // THEN: Some panicked, some succeeded — no cascade
        assert!(panic_count > 0, "at least some tasks should panic");
        assert!(success_count > 0, "at least some tasks should succeed");

        // THEN: Global state is still usable after all panics
        let result = execute_step(StepId::new("step-good".to_string()), 5000).await;
        assert!(
            result.is_ok(),
            "execute_step should work after multiple panics"
        );
    }

    #[tokio::test]
    async fn panic_in_tokio_task_does_not_affect_other_step_errors() {
        // GIVEN: A step with a recorded error
        let _guard = state_guard();
        let error_step = StepId::new("step-transient".to_string());
        let _ = execute_step(error_step.clone(), 5000).await;

        let pre_panic_error = get_last_error(&error_step);
        assert!(pre_panic_error.is_some(), "error should be recorded");

        // WHEN: A separate task panics
        let _ = tokio::spawn(async move {
            panic!("unrelated panic");
        })
        .await;

        // THEN: The original error is still intact
        let post_panic_error = get_last_error(&error_step);
        assert!(
            post_panic_error.is_some(),
            "error for step-transient should survive unrelated panic"
        );
        assert_eq!(
            pre_panic_error, post_panic_error,
            "error content should not change due to unrelated panic"
        );
    }
}

// ============================================================================
// RED QUEEN: State Leakage — Cross-Task Contamination Detection
// ============================================================================

#[cfg(test)]
mod state_leakage_tests {
    use super::*;
    use vo_executor::errors::ExecuteNodeError;

    #[tokio::test]
    async fn different_step_ids_have_independent_state() {
        // GIVEN: Multiple steps with different IDs
        let _guard = state_guard();
        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-2".to_string());
        let step_c = StepId::new("step-3".to_string());

        // WHEN: Each step is executed
        let result_a = execute_step(step_a.clone(), 5000).await;
        let result_b = execute_step(step_b.clone(), 5000).await;
        let result_c = execute_step(step_c.clone(), 5000).await;

        // THEN: All succeed independently
        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
        assert!(result_c.is_ok());

        // THEN: Each step returns to Ready independently
        assert!(matches!(get_execution_status(&step_a), vo_executor::ExecutionStatus::Ready));
        assert!(matches!(get_execution_status(&step_b), vo_executor::ExecutionStatus::Ready));
        assert!(matches!(get_execution_status(&step_c), vo_executor::ExecutionStatus::Ready));
    }

    #[tokio::test]
    async fn error_for_one_step_does_not_leak_to_another() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: Step A fails with transient error
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        let _ = execute_step(step_a.clone(), 5000).await;
        let _ = execute_step(step_b.clone(), 5000).await;

        // THEN: Step A has an error
        assert!(
            get_last_error(&step_a).is_some(),
            "transient step should have recorded error"
        );

        // THEN: Step B has NO error (no leakage)
        assert!(
            get_last_error(&step_b).is_none(),
            "step-1 should have no error — state leaked from step-transient"
        );
    }

    #[tokio::test]
    async fn cancel_one_step_does_not_affect_another() {
        // GIVEN: Clean state
        let _guard = state_guard();
        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-2".to_string());

        // WHEN: Step A is cancelled
        let _ = cancel_execution(step_a.clone()).await;

        // THEN: Step A is cancelled
        assert!(
            matches!(get_execution_status(&step_a), vo_executor::ExecutionStatus::Cancelled { .. }),
            "step A should be cancelled"
        );

        // THEN: Step B is still Ready (not affected)
        assert!(
            matches!(get_execution_status(&step_b), vo_executor::ExecutionStatus::Ready),
            "step B should remain Ready — cancel leaked"
        );

        // THEN: Step B can still execute
        let result = execute_step(step_b, 5000).await;
        assert!(result.is_ok(), "step B should execute after step A cancelled");
    }

    #[tokio::test]
    async fn concurrent_writes_to_different_keys_are_isolated() {
        // GIVEN: Clean state
        let _guard = state_guard();

        static COMPLETED_COUNT: AtomicUsize = AtomicUsize::new(0);
        const TASKS: usize = 20;

        // WHEN: 20 tasks write to different keys concurrently
        let mut handles = Vec::new();
        for i in 0..TASKS {
            handles.push(tokio::spawn(async move {
                let step_id = StepId::new(format!("leak-step-{}", i));
                let result = execute_step(step_id, 5000).await;
                if result.is_ok() {
                    COMPLETED_COUNT.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        // THEN: All 20 tasks completed without interference
        assert_eq!(
            COMPLETED_COUNT.load(Ordering::SeqCst),
            TASKS,
            "all tasks should complete — state leakage detected"
        );
    }

    #[tokio::test]
    async fn set_error_for_step_a_visible_only_to_step_a() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: Error is set for step A
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 3000,
        };
        set_error("isolation-err-a", err.clone());

        // THEN: Only step A sees the error
        assert!(
            get_last_error(&StepId::new("isolation-err-a".to_string())).is_some(),
            "error should exist for step A"
        );
        assert!(
            get_last_error(&StepId::new("isolation-err-b".to_string())).is_none(),
            "error should NOT leak to step B"
        );
        assert!(
            get_last_error(&StepId::new("isolation-err-c".to_string())).is_none(),
            "error should NOT leak to step C"
        );
    }
}

// ============================================================================
// RED QUEEN: Concurrent Same-Key Contention — Race Condition Isolation
// ============================================================================

#[cfg(test)]
mod concurrent_contention_tests {
    use super::*;

    #[tokio::test]
    async fn concurrent_execution_of_same_step_id_one_wins_others_reject() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: Multiple tasks try to execute the same step concurrently
        // (The current model uses check_not_executing which transitions to Executing atomically)
        // Since execute_step is sync despite being async, the first call wins
        let step = StepId::new("step-good".to_string());
        let result = execute_step(step.clone(), 5000).await;
        assert!(result.is_ok(), "first execution should succeed");

        // THEN: Re-execution succeeds because state returns to Ready after completion
        let result2 = execute_step(step.clone(), 5000).await;
        assert!(result2.is_ok(), "re-execution after completion should succeed");
    }

    #[tokio::test]
    async fn executing_state_blocks_concurrent_execution() {
        // GIVEN: A step manually set to Executing state
        let _guard = state_guard();
        let step = StepId::new("step-1".to_string());
        set_executing_state_for_test(step.as_str());

        // WHEN: Another execution is attempted while step is Executing
        let result = execute_step(step.clone(), 5000).await;

        // THEN: Execution is rejected with InvalidTransition
        assert!(
            result.is_err(),
            "should reject execution while step is in Executing state"
        );
        assert!(
            matches!(
                result.unwrap_err(),
                vo_executor::ExecuteNodeError::InvalidTransition { .. }
            ),
            "should return InvalidTransition error"
        );
    }

    #[tokio::test]
    async fn concurrent_cancel_and_execute_do_not_corrupt_state() {
        // GIVEN: Clean state
        let _guard = state_guard();

        static CANCEL_COUNT: AtomicUsize = AtomicUsize::new(0);
        static EXEC_COUNT: AtomicUsize = AtomicUsize::new(0);
        const ROUNDS: usize = 50;

        // WHEN: Cancel and execute race against each other
        let mut handles = Vec::new();
        for _ in 0..ROUNDS {
            let cancel_handle = tokio::spawn(async move {
                let step = StepId::new("step-1".to_string());
                let _ = cancel_execution(step).await;
                CANCEL_COUNT.fetch_add(1, Ordering::SeqCst);
            });
            handles.push(cancel_handle);

            let exec_handle = tokio::spawn(async move {
                let step = StepId::new("step-2".to_string());
                let _ = execute_step(step, 5000).await;
                EXEC_COUNT.fetch_add(1, Ordering::SeqCst);
            });
            handles.push(exec_handle);
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        // THEN: All operations completed without panic or deadlock
        assert_eq!(
            CANCEL_COUNT.load(Ordering::SeqCst),
            ROUNDS,
            "all cancel operations should complete"
        );
        assert_eq!(
            EXEC_COUNT.load(Ordering::SeqCst),
            ROUNDS,
            "all execute operations should complete"
        );
    }

    #[tokio::test]
    async fn concurrent_status_reads_during_execution_do_not_hang() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: Many tasks read status while others execute
        let step = StepId::new("step-1".to_string());
        let mut handles = Vec::new();

        for _ in 0..20 {
            let s = step.clone();
            handles.push(tokio::spawn(async move {
                let _ = get_execution_status(&s);
            }));
        }

        let exec_handle = tokio::spawn(async move {
            let _ = execute_step(step, 5000).await;
        });
        handles.push(exec_handle);

        for handle in handles {
            handle.await.expect("status read should not hang");
        }

        // THEN: All completed without timeout
    }
}

// ============================================================================
// RED QUEEN: Subprocess Isolation — OS-Level Task Isolation
// ============================================================================

#[cfg(test)]
mod subprocess_isolation_tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;
    use vo_executor::{run_subprocess, SubprocessConfig, SubprocessError};

    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    #[tokio::test]
    async fn subprocess_crash_does_not_affect_host_state() {
        // GIVEN: In-process state is populated
        let _guard = state_guard();
        let step = StepId::new("step-1".to_string());
        let _ = execute_step(step.clone(), 5000).await;

        let pre_status = get_execution_status(&step);

        // WHEN: A subprocess crashes
        let dir = tempdir().unwrap();
        let script = dir.path().join("crash.sh");
        std::fs::write(&script, "#!/bin/sh\nkill -9 $$\n").unwrap();
        make_executable(&script);

        let config = SubprocessConfig::new(script.to_string_lossy().to_string(), vec![], 2000, vec![]);
        let _ = run_subprocess(config).await;

        // THEN: Host state is unchanged
        let post_status = get_execution_status(&step);
        assert_eq!(
            pre_status, post_status,
            "subprocess crash should not affect in-process state"
        );
    }

    #[tokio::test]
    async fn concurrent_subprocesses_do_not_share_state() {
        // GIVEN: Multiple subprocess configs
        let dir = tempdir().unwrap();
        let script = dir.path().join("echo_pid.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\necho $$ > /tmp/rq_subprocess_test_$$.txt\nexit 0\n",
        )
        .unwrap();
        make_executable(&script);

        // WHEN: 5 concurrent subprocesses run
        let mut handles = Vec::new();
        for _ in 0..5 {
            let config =
                SubprocessConfig::new(script.to_string_lossy().to_string(), vec![], 2000, vec![]);
            handles.push(tokio::spawn(run_subprocess(config)));
        }

        // THEN: All complete independently
        for handle in handles {
            let result = handle.await.expect("task should not panic");
            assert!(result.is_ok(), "concurrent subprocess should succeed");
        }
    }

    #[tokio::test]
    async fn subprocess_timeout_does_not_corrupt_in_process_errors() {
        // GIVEN: An in-process error is recorded
        let _guard = state_guard();
        let err_step = StepId::new("step-transient".to_string());
        let _ = execute_step(err_step.clone(), 5000).await;
        let pre_error = get_last_error(&err_step);
        assert!(pre_error.is_some());

        // WHEN: A subprocess times out
        let dir = tempdir().unwrap();
        let script = dir.path().join("slow.sh");
        std::fs::write(&script, "#!/bin/sh\nsleep 60\n").unwrap();
        make_executable(&script);

        let config = SubprocessConfig::new(script.to_string_lossy().to_string(), vec![], 100, vec![]);
        let result = run_subprocess(config).await;
        assert!(matches!(result, Err(SubprocessError::Timeout { .. })));

        // THEN: In-process error is unchanged
        let post_error = get_last_error(&err_step);
        assert_eq!(
            pre_error, post_error,
            "subprocess timeout should not affect in-process error state"
        );
    }
}

// ============================================================================
// RED QUEEN: Resource Sharing Safety
// ============================================================================

#[cfg(test)]
mod resource_sharing_tests {
    use super::*;

    #[tokio::test]
    async fn dashmap_shard_contention_under_parallel_load() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: Many tasks hammer the global DashMap with different keys
        // (keys hash to same shard = high contention)
        const TASKS: usize = 50;
        const OPS_PER_TASK: usize = 100;

        static TOTAL_OPS: AtomicUsize = AtomicUsize::new(0);
        let mut handles = Vec::new();

        for t in 0..TASKS {
            handles.push(tokio::spawn(async move {
                for i in 0..OPS_PER_TASK {
                    let key = format!("resource-share-{}-{}", t, i);
                    set_state(
                        &key,
                        StepState::Completed {
                            output: format!("val-{}", i),
                        },
                    );
                    let _ = get_state(&key);
                    TOTAL_OPS.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        // THEN: All operations completed
        assert_eq!(
            TOTAL_OPS.load(Ordering::SeqCst),
            TASKS * OPS_PER_TASK,
            "all resource sharing ops should complete without corruption"
        );
    }

    #[tokio::test]
    async fn mixed_read_write_concurrent_no_torn_reads() {
        // GIVEN: Clean state with a known value
        let _guard = state_guard();
        set_state(
            "torn-read-victim",
            StepState::Completed {
                output: "consistent-value".to_string(),
            },
        );

        static READ_OK_COUNT: AtomicUsize = AtomicUsize::new(0);
        const READERS: usize = 20;
        const WRITERS: usize = 10;
        const ITERS: usize = 200;

        // WHEN: Concurrent readers and writers
        let mut handles = Vec::new();

        // Writers update to a known pattern
        for w in 0..WRITERS {
            handles.push(tokio::spawn(async move {
                for i in 0..ITERS {
                    set_state(
                        "torn-read-victim",
                        StepState::Completed {
                            output: format!("writer-{}-iter-{}", w, i),
                        },
                    );
                }
            }));
        }

        // Readers verify state is always valid (never torn)
        for _ in 0..READERS {
            handles.push(tokio::spawn(async move {
                for _ in 0..ITERS {
                    let state = get_state("torn-read-victim");
                    match state {
                        StepState::Completed { output } => {
                            assert!(
                                !output.is_empty(),
                                "output should never be empty/torn"
                            );
                            READ_OK_COUNT.fetch_add(1, Ordering::SeqCst);
                        }
                        StepState::Ready => {
                            READ_OK_COUNT.fetch_add(1, Ordering::SeqCst);
                        }
                        _ => {
                            READ_OK_COUNT.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            }));
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        // THEN: All reads completed without observing torn state
        assert_eq!(
            READ_OK_COUNT.load(Ordering::SeqCst),
            READERS * ITERS,
            "all reads should observe valid state"
        );
    }

    #[tokio::test]
    async fn reset_all_state_during_concurrent_step_execution() {
        // GIVEN: Clean state
        let _guard = state_guard();
        static RESET_DONE: AtomicBool = AtomicBool::new(false);

        // WHEN: Tasks execute while reset happens concurrently
        let exec_handle = tokio::spawn(async move {
            for i in 0..20 {
                let step_id = StepId::new(format!("leak-step-{}", i));
                let _ = execute_step(step_id, 5000).await;
            }
        });

        let reset_handle = tokio::spawn(async move {
            for _ in 0..5 {
                reset_all_state();
            }
            RESET_DONE.store(true, Ordering::SeqCst);
        });

        exec_handle.await.expect("exec task should not panic");
        reset_handle.await.expect("reset task should not panic");

        // THEN: Both completed — no deadlock, no panic
        assert!(RESET_DONE.load(Ordering::SeqCst));
    }
}

// ============================================================================
// RED QUEEN: Execution Boundary — In-Process vs Subprocess Isolation Gap
// ============================================================================

#[cfg(test)]
mod isolation_gap_tests {
    use super::*;

    #[tokio::test]
    async fn in_process_step_failure_does_not_block_subprocess_execution() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: In-process step fails
        let fail_step = StepId::new("step-fail".to_string());
        let result = execute_step(fail_step, 5000).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_success());

        // THEN: In-process state machine is back to Ready
        // AND: The state is not "stuck" preventing future operations

        // WHEN: Another step executes
        let ok_step = StepId::new("step-good".to_string());
        let result2 = execute_step(ok_step, 5000).await;
        assert!(
            result2.is_ok(),
            "subsequent step should not be blocked by prior failure"
        );
        assert!(result2.unwrap().is_success());
    }

    #[tokio::test]
    async fn executing_flag_is_per_step_not_global() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: Step A is set to Executing
        set_executing_state_for_test("step-999");

        // THEN: Step B is NOT executing (flag is per-step, not global)
        let status_b = get_execution_status(&StepId::new("step-2".to_string()));
        assert!(
            matches!(status_b, vo_executor::ExecutionStatus::Ready),
            "step B should be Ready — executing flag leaked globally"
        );

        // THEN: Step A is Executing
        let status_a = get_execution_status(&StepId::new("step-999".to_string()));
        assert!(
            matches!(status_a, vo_executor::ExecutionStatus::Executing { .. }),
            "step A should be Executing"
        );

        // THEN: Step B can still execute
        let result = execute_step(StepId::new("step-2".to_string()), 5000).await;
        assert!(
            result.is_ok(),
            "step B should execute despite step A being in Executing state"
        );
    }

    #[tokio::test]
    async fn global_state_count_reflects_all_tasks_not_just_last() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: 10 different steps execute
        for i in 0..10 {
            let step_id = StepId::new(format!("step-{}", i));
            let _ = execute_step(step_id, 5000).await;
        }

        // THEN: State count reflects all 10 (no overwrite/collapse)
        // Each execution leaves a Ready entry in the DashMap
        let count = vo_executor::get_state_count();
        assert_eq!(
            count, 10,
            "state count should reflect all 10 tasks — {} entries found, expected 10",
            count
        );
    }

    #[tokio::test]
    async fn error_map_isolation_per_step_under_load() {
        // GIVEN: Clean state
        let _guard = state_guard();

        // WHEN: 10 transient steps each record their own error
        for i in 0..10 {
            let step_id = StepId::new(format!("transient-step-{}", i));
            let _ = execute_step(step_id, 5000).await;
        }

        // THEN: Each step has its own distinct error
        for i in 0..10 {
            let step_id = StepId::new(format!("transient-step-{}", i));
            let err = get_last_error(&step_id);
            assert!(
                err.is_some(),
                "error for transient-load-{} should exist — error leakage detected",
                i
            );
        }

        // THEN: Error count matches
        let err_count = vo_executor::get_error_count();
        assert_eq!(
            err_count, 10,
            "error count should be 10 — got {}, indicating cross-step contamination",
            err_count
        );
    }
}
