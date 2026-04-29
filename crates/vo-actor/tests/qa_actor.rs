//! QA actor lifecycle tests.
//!
//! Covers: actor creation, supervision hierarchy, message routing, graceful shutdown.

use vo_actor::lifecycle::{ActorLifecycleState, LifecycleTransition, ParentChildRegistry};
use vo_types::InstanceId;

#[test]
fn full_happy_path_lifecycle() {
    let s = ActorLifecycleState::Pending;
    assert_eq!(
        vo_actor::lifecycle::compute_next_state(s, LifecycleTransition::Start),
        Some(ActorLifecycleState::Running)
    );
    assert_eq!(
        vo_actor::lifecycle::compute_next_state(
            ActorLifecycleState::Running,
            LifecycleTransition::Stop
        ),
        Some(ActorLifecycleState::Stopping)
    );
    assert_eq!(
        vo_actor::lifecycle::compute_next_state(
            ActorLifecycleState::Stopping,
            LifecycleTransition::AllChildrenStopped
        ),
        Some(ActorLifecycleState::Stopped)
    );
}

#[test]
fn failure_from_pending_and_running_states() {
    // Stopping does NOT accept Fail — it must complete graceful shutdown
    for state in [ActorLifecycleState::Pending, ActorLifecycleState::Running] {
        assert_eq!(
            vo_actor::lifecycle::compute_next_state(state, LifecycleTransition::Fail),
            Some(ActorLifecycleState::Failed),
            "{state:?} should transition to Failed"
        );
    }
    assert_eq!(
        vo_actor::lifecycle::compute_next_state(
            ActorLifecycleState::Stopping,
            LifecycleTransition::Fail
        ),
        None,
        "Stopping should reject Fail — must complete shutdown"
    );
}

#[tokio::test]
async fn parent_child_add_and_query() {
    let reg = ParentChildRegistry::new();
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    reg.add_child(id.clone()).await;

    assert_eq!(reg.active_children_count().await, 1);
    assert!(!reg.all_children_terminal().await);

    let pending = reg
        .get_children_by_state(ActorLifecycleState::Pending)
        .await;
    assert_eq!(pending, vec![id]);
}

#[tokio::test]
async fn shutdown_propagation_all_children_stopped() {
    let reg = ParentChildRegistry::new();
    let ids: Vec<InstanceId> = ["01H5JYV4XHGSR2F8KZ9BWNRFMA", "01H5JYV4XHGSR2F8KZ9BWNRFMB"]
        .map(|s| InstanceId::parse(s).unwrap())
        .to_vec();

    for id in &ids {
        reg.add_child(id.clone()).await;
    }
    assert_eq!(reg.active_children_count().await, 2);

    for id in &ids {
        reg.update_child_state(id, ActorLifecycleState::Stopped)
            .await;
    }

    assert!(reg.all_children_terminal().await);
    assert_eq!(reg.active_children_count().await, 0);
}

#[tokio::test]
async fn message_routing_unknown_channel_returns_error() {
    let config = vo_actor::message_router::RouterConfig::new(
        10,
        100,
        std::time::Duration::from_secs(5),
        true,
    );
    let mut router = vo_actor::message_router::MessageRouter::new(config);
    let ch = vo_actor::message_router::ChannelId::new("no-such-channel");

    let msg = vo_actor::message_router::TypedMessage::new("payload");
    let result = router.route(&ch, msg).await;
    assert!(result.is_err());
    // ChannelNotFound is a validation error, not a DLQ-enqueued delivery failure
    assert_eq!(router.dlq_depth(), 0);
}

#[test]
fn shutdown_propagator_config() {
    let prop = vo_actor::lifecycle::ShutdownPropagator::new(
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(10),
    );
    assert_eq!(prop.graceful_timeout(), std::time::Duration::from_secs(30));
    assert_eq!(
        prop.force_kill_timeout(),
        std::time::Duration::from_secs(10)
    );
}
