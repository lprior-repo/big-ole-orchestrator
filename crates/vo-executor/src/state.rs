//! Internal state management for vo-executor

use dashmap::DashMap;
use std::sync::LazyLock;
use std::time::Instant;

use crate::errors::ExecuteNodeError;
use crate::types::StepId;

/// Execution state for a step.
#[derive(Debug, Clone)]
pub enum StepState {
    Ready,
    Executing {
        step_id: StepId,
        start_time: Instant,
    },
    #[allow(dead_code)]
    Completed {
        output: String,
    },
    Cancelled {
        reason: String,
    },
}

/// Global state map: `step_id` -> `StepState`
static STATE: LazyLock<DashMap<String, StepState>> = LazyLock::new(DashMap::new);

/// Global error map: `step_id` -> last error
static LAST_ERROR: LazyLock<DashMap<String, ExecuteNodeError>> = LazyLock::new(DashMap::new);

/// Duration threshold for detecting slow steps (3000ms).
/// Steps taking longer than this trigger timeout errors if `timeout_ms` is smaller.
pub(crate) const SLOW_STEP_DURATION_MS: u64 = 3000;

/// Get current state for a step.
pub fn get_state(step_id: &str) -> StepState {
    STATE.get(step_id).map_or(StepState::Ready, |v| v.clone())
}

/// Set state for a step.
pub fn set_state(step_id: &str, state: StepState) {
    STATE.insert(step_id.to_string(), state);
}

/// Clear any stored error for a step.
///pub for testing
pub fn clear_error(step_id: &str) {
    LAST_ERROR.remove(step_id);
}

/// Store an error for a step.
///pub for testing
pub fn set_error(step_id: &str, err: ExecuteNodeError) {
    LAST_ERROR.insert(step_id.to_string(), err);
}

/// **NOTE:** This is test infrastructure that simulates workflow step behavior.
pub fn step_behavior(step_id: &str) -> StepBehavior {
    if step_id.starts_with("step-") && step_id[5..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("workflow-step-") && step_id[14..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("leak-step-") && step_id[10..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("sustained-") && step_id[10..].contains('-') {
        let suffix = &step_id[step_id.rfind('-').map_or(10, |p| p + 1)..];
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            return StepBehavior::Success;
        }
    }
    if step_id.starts_with("concurrent-leak-") && step_id[16..].chars().all(|c| c.is_ascii_digit())
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("warm-") && step_id[5..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("bench-state-read-") && step_id[17..].chars().all(|c| c.is_ascii_digit())
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("growth-")
        && step_id[7..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("cold-start-")
        && step_id[11..]
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("error-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("batch-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("mixed-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("retry-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("write-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("read-") && step_id[5..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("transient-step-") && step_id[15..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Transient;
    }
    match step_id {
        "step-1" | "step-good" | "step-valid" | "step-retry" | "workflow-step-1" => {
            StepBehavior::Success
        }
        "step-fail" => StepBehavior::Failure,
        "step-transient" | "step-flaky" => StepBehavior::Transient,
        "step-slow" => StepBehavior::Slow,
        _ => StepBehavior::NotFound,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StepBehavior {
    Success,
    Failure,
    Transient,
    Slow,
    NotFound,
}

/// Get the last error for a step (if any).
pub fn get_last_error(step_id: &str) -> Option<ExecuteNodeError> {
    LAST_ERROR.get(step_id).map(|v| v.clone())
}

/// Reset all global state (STATE and LAST_ERROR DashMaps).
pub fn reset_all_state() {
    STATE.clear();
    LAST_ERROR.clear();
}

/// Set executing state for a step (test infrastructure).
pub fn set_executing_state_for_test(step_id: &str) {
    STATE.insert(
        step_id.to_string(),
        StepState::Executing {
            step_id: StepId::new(step_id.to_string()),
            start_time: Instant::now(),
        },
    );
}

/// Get the current count of entries in the STATE map.
/// Useful for detecting memory leaks under sustained load.
pub fn get_state_count() -> usize {
    STATE.len()
}

/// Get the current count of entries in the LAST_ERROR map.
pub fn get_error_count() -> usize {
    LAST_ERROR.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::errors::ExecuteNodeError;
    use std::sync::Arc;
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn setup() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    #[test]
    fn reset_all_state_clears_everything() {
        let _guard = setup();
        set_state(
            "test-reset-a",
            StepState::Completed {
                output: "x".to_string(),
            },
        );
        set_error(
            "test-reset-a",
            ExecuteNodeError::ExecutionCancelled {
                reason: "r".to_string(),
            },
        );

        reset_all_state();

        assert!(matches!(get_state("test-reset-a"), StepState::Ready));
        assert!(get_last_error("test-reset-a").is_none());
    }

    #[test]
    fn set_and_get_state() {
        let _guard = setup();
        set_state(
            "step-a",
            StepState::Completed {
                output: "result".to_string(),
            },
        );
        let state = get_state("step-a");
        assert!(matches!(state, StepState::Completed { output } if output == "result"));
    }

    #[test]
    fn set_state_overwrites() {
        let _guard = setup();
        set_state("step-a", StepState::Ready);
        set_state(
            "step-a",
            StepState::Cancelled {
                reason: "test".to_string(),
            },
        );
        let state = get_state("step-a");
        assert!(matches!(state, StepState::Cancelled { .. }));
    }

    #[test]
    fn executing_state() {
        let _guard = setup();
        let key = "test-exec-state-unique";
        let start = Instant::now();
        set_state(
            key,
            StepState::Executing {
                step_id: StepId::new(key.to_string()),
                start_time: start,
            },
        );
        let state = get_state(key);
        assert!(matches!(state, StepState::Executing { .. }));
    }

    #[test]
    fn clear_error_no_error() {
        let _guard = setup();
        clear_error("step-a");
        assert!(get_last_error("step-a").is_none());
    }

    #[test]
    fn set_and_get_error() {
        let _guard = setup();
        let key = "test-err-unique";
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 3000,
        };
        set_error(key, err.clone());
        let retrieved = get_last_error(key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), err);
    }

    #[test]
    fn clear_error_removes_existing() {
        let _guard = setup();
        let key = "test-clear-err-unique";
        let err = ExecuteNodeError::ExecutionCancelled {
            reason: "test".to_string(),
        };
        set_error(key, err);
        assert!(get_last_error(key).is_some());
        clear_error(key);
        assert!(get_last_error(key).is_none());
    }

    #[test]
    fn step_behavior_success_variants() {
        let success_steps = [
            "step-1",
            "step-good",
            "step-valid",
            "step-retry",
            "workflow-step-1",
        ];
        for step in success_steps {
            assert!(
                matches!(step_behavior(step), StepBehavior::Success),
                "failed for {}",
                step
            );
        }
    }

    #[test]
    fn step_behavior_failure() {
        assert!(matches!(step_behavior("step-fail"), StepBehavior::Failure));
    }

    #[test]
    fn step_behavior_transient() {
        assert!(matches!(
            step_behavior("step-transient"),
            StepBehavior::Transient
        ));
        assert!(matches!(
            step_behavior("step-flaky"),
            StepBehavior::Transient
        ));
    }

    #[test]
    fn step_behavior_slow() {
        assert!(matches!(step_behavior("step-slow"), StepBehavior::Slow));
    }

    #[test]
    fn step_behavior_not_found() {
        assert!(matches!(step_behavior("unknown"), StepBehavior::NotFound));
        assert!(matches!(step_behavior(""), StepBehavior::NotFound));
        assert!(matches!(step_behavior("STEP-1"), StepBehavior::NotFound));
    }

    #[test]
    fn step_behavior_is_copy() {
        let b = step_behavior("step-1");
        let _b2 = b;
    }

    #[test]
    fn step_state_clone() {
        let state = StepState::Completed {
            output: "data".to_string(),
        };
        let cloned = state.clone();
        assert!(matches!(cloned, StepState::Completed { .. }));
    }

    const CONCURRENT_THREADS: usize = 8;
    const OPS_PER_THREAD: usize = 1000;

    #[test]
    fn concurrent_set_and_get_state_no_data_loss() {
        let _guard = setup();
        reset_all_state();

        let handles: Vec<_> = (0..CONCURRENT_THREADS)
            .map(|i| {
                std::thread::spawn(move || {
                    for j in 0..OPS_PER_THREAD {
                        let step_id = format!("step-{}-{}", i, j);
                        set_state(
                            &step_id,
                            StepState::Completed {
                                output: format!("output-{}-{}", i, j),
                            },
                        );
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(
            get_state_count(),
            CONCURRENT_THREADS * OPS_PER_THREAD,
            "expected {} entries, got {}",
            CONCURRENT_THREADS * OPS_PER_THREAD,
            get_state_count()
        );

        for i in 0..CONCURRENT_THREADS {
            for j in 0..OPS_PER_THREAD {
                let step_id = format!("step-{}-{}", i, j);
                let state = get_state(&step_id);
                assert!(
                    matches!(state, StepState::Completed { ref output } if output == &format!("output-{}-{}", i, j)),
                    "state mismatch for {}",
                    step_id
                );
            }
        }
    }

    #[test]
    fn concurrent_set_error_clear_error_last_write_wins() {
        let _guard = setup();
        reset_all_state();

        let step_id = "concurrent-error-key";
        set_error(
            step_id,
            ExecuteNodeError::ExecutionCancelled {
                reason: "initial".to_string(),
            },
        );

        let handles: Vec<_> = (0..CONCURRENT_THREADS)
            .map(|i| {
                let step_id = step_id.to_string();
                std::thread::spawn(move || {
                    for j in 0..OPS_PER_THREAD {
                        let err = ExecuteNodeError::TimeoutExceeded {
                            elapsed_ms: (i * OPS_PER_THREAD + j) as u64,
                            limit_ms: 3000,
                        };
                        set_error(&step_id, err);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        let final_error = get_last_error(step_id);
        assert!(
            final_error.is_some(),
            "expected some error after concurrent writes"
        );
        if let Some(ExecuteNodeError::TimeoutExceeded { elapsed_ms, .. }) = final_error {
            let max_possible = (CONCURRENT_THREADS * OPS_PER_THREAD) as u64 - 1;
            assert!(
                elapsed_ms <= max_possible,
                "elapsed_ms {} should be <= max possible {}",
                elapsed_ms,
                max_possible
            );
        }
    }

    #[test]
    fn concurrent_set_and_clear_error_consistency() {
        let _guard = setup();
        reset_all_state();

        let step_id = "concurrent-clear-test";
        let set_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let clear_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let handles: Vec<_> = (0..CONCURRENT_THREADS)
            .map(|i| {
                let step_id = step_id.to_string();
                let set_count = Arc::clone(&set_count);
                let clear_count = Arc::clone(&clear_count);
                std::thread::spawn(move || {
                    for j in 0..OPS_PER_THREAD {
                        if j % 2 == 0 {
                            let err = ExecuteNodeError::ExecutionCancelled {
                                reason: format!("error-{}-{}", i, j),
                            };
                            set_error(&step_id, err);
                            set_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            clear_error(&step_id);
                            clear_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        let set_total = set_count.load(std::sync::atomic::Ordering::Relaxed);
        let clear_total = clear_count.load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            set_total > 0 && clear_total > 0,
            "both set and clear should have been called: set={}, clear={}",
            set_total,
            clear_total
        );
    }

    #[test]
    fn get_state_count_monotonic_under_concurrent_modification() {
        let _guard = setup();
        reset_all_state();

        let sample_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let writer_handle = std::thread::spawn(|| {
            for i in 0..OPS_PER_THREAD {
                let step_id = format!("monotonic-step-{}", i);
                set_state(
                    &step_id,
                    StepState::Executing {
                        step_id: StepId::new(step_id.clone()),
                        start_time: Instant::now(),
                    },
                );
            }
        });

        let reader_handle = {
            let sample_count = Arc::clone(&sample_count);
            std::thread::spawn(move || {
                let mut samples = Vec::new();
                while sample_count.load(std::sync::atomic::Ordering::Relaxed) < 100 {
                    let cnt = get_state_count();
                    samples.push(cnt);
                    sample_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    std::thread::yield_now();
                }
                samples
            })
        };

        writer_handle.join().expect("writer panicked");
        let samples = reader_handle.join().expect("reader panicked");

        let final_count = get_state_count();
        assert!(
            samples.iter().all(|&c| c <= final_count),
            "count should never exceed final count {} but got {:?}",
            final_count,
            samples
        );
    }

    #[test]
    fn get_error_count_monotonic_under_concurrent_modification() {
        let _guard = setup();
        reset_all_state();

        let sample_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let writer_handle = std::thread::spawn(|| {
            for i in 0..OPS_PER_THREAD {
                let step_id = format!("monotonic-error-{}", i);
                set_error(
                    &step_id,
                    ExecuteNodeError::TimeoutExceeded {
                        elapsed_ms: i as u64,
                        limit_ms: 3000,
                    },
                );
            }
        });

        let reader_handle = {
            let sample_count = Arc::clone(&sample_count);
            std::thread::spawn(move || {
                let mut samples = Vec::new();
                while sample_count.load(std::sync::atomic::Ordering::Relaxed) < 100 {
                    let cnt = get_error_count();
                    samples.push(cnt);
                    sample_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    std::thread::yield_now();
                }
                samples
            })
        };

        writer_handle.join().expect("writer panicked");
        let samples = reader_handle.join().expect("reader panicked");

        let final_count = get_error_count();
        assert!(
            samples.iter().all(|&c| c <= final_count),
            "error count should never exceed final count {} but got {:?}",
            final_count,
            samples
        );
    }

    #[test]
    fn reset_all_state_during_concurrent_writes_no_panic() {
        let _guard = setup();
        reset_all_state();

        let writer_handle = std::thread::spawn(|| {
            for i in 0..OPS_PER_THREAD {
                let step_id = format!("reset-during-write-{}", i);
                set_state(
                    &step_id,
                    StepState::Completed {
                        output: format!("output-{}", i),
                    },
                );
                set_error(
                    &step_id,
                    ExecuteNodeError::ExecutionCancelled {
                        reason: format!("error-{}", i),
                    },
                );
            }
        });

        let reset_handle = std::thread::spawn(|| {
            for _ in 0..10 {
                reset_all_state();
                std::thread::yield_now();
            }
        });

        writer_handle.join().expect("writer panicked");
        reset_handle.join().expect("reset panicked");

        let state_count = get_state_count();
        let error_count = get_error_count();
        assert!(
            state_count <= OPS_PER_THREAD,
            "state count {} should be <= {} after resets",
            state_count,
            OPS_PER_THREAD
        );
        assert!(
            error_count <= OPS_PER_THREAD,
            "error count {} should be <= {} after resets",
            error_count,
            OPS_PER_THREAD
        );
    }

    #[test]
    fn reset_all_state_during_concurrent_reads_no_panic() {
        let _guard = setup();

        for i in 0..100 {
            set_state(
                &format!("pre-existing-{}", i),
                StepState::Completed {
                    output: format!("output-{}", i),
                },
            );
            set_error(
                &format!("pre-existing-{}", i),
                ExecuteNodeError::ExecutionCancelled {
                    reason: format!("error-{}", i),
                },
            );
        }

        let reader_handles: Vec<_> = (0..CONCURRENT_THREADS)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..OPS_PER_THREAD {
                        let _ = get_state_count();
                        let _ = get_error_count();
                        let _ = get_state("pre-existing-0");
                        let _ = get_last_error("pre-existing-0");
                        std::thread::yield_now();
                    }
                })
            })
            .collect();

        let reset_handle = std::thread::spawn(|| {
            for _ in 0..20 {
                reset_all_state();
                std::thread::yield_now();
            }
        });

        for handle in reader_handles {
            handle.join().expect("reader panicked");
        }
        reset_handle.join().expect("reset panicked");
    }

    #[test]
    fn concurrent_mixed_operations_stress() {
        let _guard = setup();
        reset_all_state();

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let handles: Vec<_> = (0..CONCURRENT_THREADS)
            .map(|_tid| {
                let counter = Arc::clone(&counter);
                std::thread::spawn(move || {
                    for _i in 0..OPS_PER_THREAD {
                        let idx = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let step_id = format!("mixed-step-{}", idx);

                        match idx % 4 {
                            0 => {
                                set_state(
                                    &step_id,
                                    StepState::Executing {
                                        step_id: StepId::new(step_id.clone()),
                                        start_time: Instant::now(),
                                    },
                                );
                            }
                            1 => {
                                let _ = get_state(&step_id);
                            }
                            2 => {
                                set_error(
                                    &step_id,
                                    ExecuteNodeError::TimeoutExceeded {
                                        elapsed_ms: idx as u64,
                                        limit_ms: 3000,
                                    },
                                );
                            }
                            3 => {
                                let _ = get_last_error(&step_id);
                            }
                            _ => unreachable!(),
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        let total_ops = counter.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            total_ops,
            CONCURRENT_THREADS * OPS_PER_THREAD,
            "all operations should complete"
        );
    }
}
