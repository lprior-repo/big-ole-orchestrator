//! CONCURRENT-005: Stress test — 100 concurrent tasks mutate state without panics

#[tokio::test]
async fn bh_stress_concurrent_state_mutations_100_tasks() {
    use std::time::Instant;
    use vo_executor::{reset_all_state, StepId};
    use vo_executor::state::{get_state, set_state, StepState};

    reset_all_state();

    let mut handles = Vec::new();
    for i in 0..100u32 {
        handles.push(tokio::spawn(async move {
            let step = StepId::new(format!("bh-stress-{:03}", i));
            set_state(step.as_str(), StepState::Executing { step_id: step.clone(), start_time: Instant::now() });
            let state = get_state(step.as_str());
            set_state(step.as_str(), StepState::Ready);
            assert!(matches!(state, StepState::Executing { .. }));
        }));
    }

    for h in handles {
        assert!(h.await.is_ok(), "task panicked");
    }
}
