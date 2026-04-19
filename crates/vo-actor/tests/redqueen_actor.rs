//! RED-QUEEN coevolutionary adversarial tests for vo-actor.
//! Lifecycle state machine, supervision tree, DLQ overflow,
//! instance registry atomicity, failure scope boundaries.

use std::time::Duration;
use vo_actor::fairness::WorkloadClass;
use vo_actor::instance_registry::*;
use vo_actor::lifecycle::*;
use vo_actor::message_router::*;
use vo_types::signal::FailureScope;
use vo_types::InstanceId;

const ALL_TRANS: [LifecycleTransition; 5] = [
    LifecycleTransition::Start,
    LifecycleTransition::Stop,
    LifecycleTransition::Fail,
    LifecycleTransition::ChildStopped,
    LifecycleTransition::AllChildrenStopped,
];

#[test]
fn rq_no_transition_from_terminal_stopped() {
    for t in &ALL_TRANS {
        assert!(compute_next_state(ActorLifecycleState::Stopped, *t).is_none());
    }
}

#[test]
fn rq_no_transition_from_terminal_failed() {
    for t in &ALL_TRANS {
        assert!(compute_next_state(ActorLifecycleState::Failed, *t).is_none());
    }
}

#[test]
fn rq_valid_transitions_roundtrip() {
    for (from, t, exp) in [
        (
            ActorLifecycleState::Pending,
            LifecycleTransition::Start,
            ActorLifecycleState::Running,
        ),
        (
            ActorLifecycleState::Pending,
            LifecycleTransition::Fail,
            ActorLifecycleState::Failed,
        ),
        (
            ActorLifecycleState::Running,
            LifecycleTransition::Stop,
            ActorLifecycleState::Stopping,
        ),
        (
            ActorLifecycleState::Running,
            LifecycleTransition::Fail,
            ActorLifecycleState::Failed,
        ),
        (
            ActorLifecycleState::Stopping,
            LifecycleTransition::ChildStopped,
            ActorLifecycleState::Stopping,
        ),
        (
            ActorLifecycleState::Stopping,
            LifecycleTransition::AllChildrenStopped,
            ActorLifecycleState::Stopped,
        ),
    ] {
        assert_eq!(compute_next_state(from, t).unwrap(), exp);
        assert!(is_valid_transition(from, t));
    }
}

#[test]
fn rq_failure_scope_epoch_keeps_lineage_active() {
    for s in [
        ActorLifecycleState::Pending,
        ActorLifecycleState::Running,
        ActorLifecycleState::Stopping,
    ] {
        let o = compute_failure_outcome(s, FailureScope::Epoch);
        assert!(o.is_epoch_failure() && o.can_lineage_spawn_epoch());
    }
}

#[test]
fn rq_failure_scope_lineage_tombstones() {
    let o = compute_failure_outcome(ActorLifecycleState::Running, FailureScope::Lineage);
    assert!(o.is_lineage_failure() && !o.can_lineage_spawn_epoch());
}

// ── Supervision Tree ──

#[tokio::test]
async fn rq_orphan_child_removed_without_update() {
    let reg = ParentChildRegistry::new();
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    reg.add_child(id.clone()).await;
    reg.remove_child(&id).await;
    assert_eq!(reg.active_children_count().await, 0);
    assert!(reg.all_children_terminal().await);
}

#[tokio::test]
async fn rq_empty_registry_reports_all_terminal() {
    let reg = ParentChildRegistry::new();
    assert!(reg.all_children_terminal().await && reg.all_children_stopped().await);
}

#[tokio::test]
async fn rq_failed_child_is_terminal_not_stopping() {
    let reg = ParentChildRegistry::new();
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    reg.add_child(id.clone()).await;
    reg.update_child_state(&id, ActorLifecycleState::Failed)
        .await;
    assert!(reg.all_children_terminal().await && !reg.all_children_stopped().await);
}

// ── Dead Letter Queue ──

#[test]
fn rq_dlq_evicts_oldest_when_full() {
    let mut dlq = DeadLetterQueue::new(3);
    let ch = ChannelId::new("x");
    for i in 0..5u8 {
        dlq.enqueue(DeadLetterEntry {
            channel_id: ch.clone(),
            message: DeadLetterMessage::new(&i).unwrap(),
            enqueued_at: TimestampMs::now(),
            reason: DeadLetterReason::ChannelNotFound,
        });
    }
    assert_eq!(dlq.len(), 3);
    assert_eq!(
        dlq.dequeue().unwrap().message.deserialize::<u8>().unwrap(),
        2
    );
}

#[test]
fn rq_dlq_empty_dequeue_returns_none() {
    assert!(DeadLetterQueue::new(10).dequeue().is_none());
}

// ── Instance Registry ──

#[test]
fn rq_registry_stop_fn_failure_rolls_back() {
    let mut reg = InstanceRegistry::new(RegistryConfig::default());
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    reg.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert!(reg
        .register(id.clone(), InstanceActorHandle::test(2), |_| Err(
            "refused".into()
        ))
        .is_err());
    assert_eq!(reg.lookup(&id).unwrap().handle_id(), 1);
}

#[test]
fn rq_registry_stop_fn_timeout_rolls_back() {
    let mut reg = InstanceRegistry::new(RegistryConfig {
        stop_timeout: Duration::from_millis(1),
    });
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    reg.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let r = reg.register(id.clone(), InstanceActorHandle::test(2), |_| {
        std::thread::sleep(Duration::from_millis(100));
        Ok(())
    });
    assert!(matches!(r, Err(RegistryError::StopTimeout { .. })));
    assert_eq!(reg.lookup(&id).unwrap().handle_id(), 1);
}

#[test]
fn rq_registry_deregister_unknown_errors() {
    let mut reg = InstanceRegistry::new(RegistryConfig::default());
    assert!(reg
        .deregister(&InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap())
        .is_err());
}

// ── ReservedPermitBudget ──

#[test]
fn rq_budget_exhaustion_then_release_allows() {
    let mut b = vo_actor::ReservedPermitBudget::new(1);
    b.try_acquire(WorkloadClass::Recovery).unwrap();
    assert!(matches!(
        b.try_acquire(WorkloadClass::Recovery),
        Err(vo_actor::StartError::BudgetExhaustion { available: 0, .. })
    ));
    b.release(WorkloadClass::Recovery);
    assert!(b.try_acquire(WorkloadClass::Recovery).is_ok());
}

#[test]
fn rq_budget_cross_class_isolation() {
    let mut b = vo_actor::ReservedPermitBudget::new(1);
    b.try_acquire(WorkloadClass::Recovery).unwrap();
    assert!(b.try_acquire(WorkloadClass::NewInstance).is_ok());
    assert!(b.is_exhausted(WorkloadClass::Recovery));
}
