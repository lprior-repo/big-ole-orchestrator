use std::sync::Arc;
use std::time::Duration;
use vo_actor::semaphore::{
    calculate_backpressure_status, estimate_wait_ms, is_workflow_saturated,
    BackpressureStatus, ExecutionSemaphore, SemaphoreConfig, WorkflowSemaphoreMap,
};
use vo_types::WorkflowName;

#[tokio::test]
async fn execution_semaphore_fairness_fifo_order() {
    let sem = Arc::new(ExecutionSemaphore::default());
    let mut handles = Vec::new();

    for i in 0..5 {
        let sem_clone = sem.clone();
        let handle = tokio::spawn(async move {
            let decision = sem_clone.acquire().await;
            (i, decision)
        });
        handles.push(handle);
    }

    let results: Vec<Result<(usize, _), _>> = futures::future::join_all(handles).await;
    for result in results {
        let (_i, decision) = result.unwrap();
        assert!(matches!(decision, vo_actor::semaphore::AdmissionDecision::Admitted));
    }
}

#[tokio::test]
async fn execution_semaphore_concurrent_limit_enforced() {
    let config = SemaphoreConfig {
        max_concurrent_binaries: 2,
        max_waiters_for_shed: 1000,
        max_per_workflow: 10,
        acquire_timeout: Duration::from_secs(30),
        reserved_permits: 0,
        ..Default::default()
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));

    let permit1 = sem.try_acquire();
    let permit2 = sem.try_acquire();
    let permit3 = sem.try_acquire();

    assert!(permit1.is_some());
    assert!(permit2.is_some());
    assert!(permit3.is_none());
}

#[test]
fn backpressure_status_healthy_under_threshold() {
    let status = calculate_backpressure_status(400, 500, 100, 5000);
    assert_eq!(status, BackpressureStatus::Healthy);
}

#[test]
fn backpressure_status_heavy_over_half_waiters() {
    let status = calculate_backpressure_status(100, 500, 300, 5000);
    assert_eq!(status, BackpressureStatus::Heavy);
}

#[test]
fn backpressure_status_shed_load_at_max_waiters() {
    let status = calculate_backpressure_status(0, 500, 5000, 5000);
    assert_eq!(status, BackpressureStatus::ShedLoad);
}

#[test]
fn backpressure_status_heavy_at_high_usage_ratio() {
    let status = calculate_backpressure_status(50, 500, 0, 5000);
    assert_eq!(status, BackpressureStatus::Heavy);
}

#[test]
fn estimate_wait_ms_with_parallelism() {
    let wait = estimate_wait_ms(10, 5, 100);
    assert_eq!(wait, 300);
}

#[test]
fn estimate_wait_ms_no_permits() {
    let wait = estimate_wait_ms(5, 0, 100);
    assert_eq!(wait, 600);
}

#[test]
fn workflow_saturation_detection() {
    assert!(!is_workflow_saturated(5, 10));
    assert!(is_workflow_saturated(10, 10));
    assert!(is_workflow_saturated(15, 10));
}

#[tokio::test]
async fn workflow_semaphore_map_isolates_workflows() {
    let map = WorkflowSemaphoreMap::default();
    let wf_a = WorkflowName::parse("workflow-a").unwrap();
    let wf_b = WorkflowName::parse("workflow-b").unwrap();

    let sem_a1 = map.semaphore_for(&wf_a);
    let sem_a2 = map.semaphore_for(&wf_a);
    let sem_b = map.semaphore_for(&wf_b);

    assert_eq!(map.len(), 2);
    assert!(Arc::ptr_eq(&sem_a1, &sem_a2));
    assert!(!Arc::ptr_eq(&sem_a1, &sem_b));
}

#[tokio::test]
async fn workflow_semaphore_map_respects_max_per_workflow() {
    let map = WorkflowSemaphoreMap::new(2);
    let wf = WorkflowName::parse("test-workflow").unwrap();

    let sem = map.semaphore_for(&wf);
    let permit1 = sem.try_acquire().ok();
    let permit2 = sem.try_acquire().ok();
    let permit3 = sem.try_acquire().ok();

    assert!(permit1.is_some());
    assert!(permit2.is_some());
    assert!(permit3.is_none());
}

#[test]
fn backpressure_status_moderate_with_higher_usage() {
    let status = calculate_backpressure_status(200, 500, 100, 5000);
    assert_eq!(status, BackpressureStatus::Moderate);
}
