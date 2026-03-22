use super::*;
use bytes::Bytes;

fn dispatch(id: &str) -> WorkflowEvent {
    WorkflowEvent::ActivityDispatched {
        activity_id: id.into(),
        activity_type: "work".into(),
        payload: Bytes::new(),
        retry_policy: wtf_common::RetryPolicy::default(),
        attempt: 1,
    }
}

fn complete(id: &str, result: &[u8]) -> WorkflowEvent {
    WorkflowEvent::ActivityCompleted {
        activity_id: id.into(),
        result: Bytes::copy_from_slice(result),
        duration_ms: 5,
    }
}

fn fail(id: &str, exhausted: bool) -> WorkflowEvent {
    WorkflowEvent::ActivityFailed {
        activity_id: id.into(),
        error: "oops".into(),
        retries_exhausted: exhausted,
    }
}

#[test]
fn new_state_has_zero_counter() {
    let s = ProceduralActorState::new();
    assert_eq!(s.operation_counter, 0);
    assert!(s.checkpoint_map.is_empty());
    assert!(s.in_flight.is_empty());
}

#[test]
fn dispatch_increments_operation_counter() {
    let s0 = ProceduralActorState::new();
    let (s1, result) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
    assert_eq!(s1.operation_counter, 1);
    assert!(matches!(
        result,
        ProceduralApplyResult::ActivityDispatched {
            operation_id: 0,
            ..
        }
    ));
    assert!(s1.in_flight.contains_key(&0));
}

#[test]
fn complete_writes_checkpoint() {
    let s0 = ProceduralActorState::new();
    let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
    let (s2, result) = apply_event(&s1, &complete("act-1", b"result"), 2).expect("complete");

    assert!(matches!(
        result,
        ProceduralApplyResult::ActivityCompleted {
            operation_id: 0,
            ..
        }
    ));
    assert!(s2.checkpoint_map.contains_key(&0));
    assert_eq!(s2.checkpoint_map[&0].result, Bytes::from_static(b"result"));
    assert!(!s2.in_flight.contains_key(&0));
}

#[test]
fn sequential_operations_get_incrementing_ids() {
    let s0 = ProceduralActorState::new();
    let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch 1");
    let (s2, _) = apply_event(&s1, &complete("act-1", b"r1"), 2).expect("complete 1");
    let (s3, _) = apply_event(&s2, &dispatch("act-2"), 3).expect("dispatch 2");
    let (s4, _) = apply_event(&s3, &complete("act-2", b"r2"), 4).expect("complete 2");

    assert_eq!(s4.operation_counter, 2);
    assert!(s4.checkpoint_map.contains_key(&0));
    assert!(s4.checkpoint_map.contains_key(&1));
}

#[test]
fn duplicate_seq_returns_already_applied() {
    let s0 = ProceduralActorState::new();
    let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("first");
    let (_, result) = apply_event(&s1, &dispatch("act-1"), 1).expect("dup");
    assert!(matches!(result, ProceduralApplyResult::AlreadyApplied));
}

#[test]
fn has_checkpoint_reflects_completed_ops() {
    let s0 = ProceduralActorState::new();
    assert!(!s0.has_checkpoint(0));
    let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
    let (s2, _) = apply_event(&s1, &complete("act-1", b"r"), 2).expect("complete");
    assert!(s2.has_checkpoint(0));
}

#[test]
fn max_checkpointed_operation_id_returns_highest() {
    let s0 = ProceduralActorState::new();
    assert_eq!(s0.max_checkpointed_operation_id(), None);

    let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("d1");
    let (s2, _) = apply_event(&s1, &complete("act-1", b"r"), 2).expect("c1");
    let (s3, _) = apply_event(&s2, &dispatch("act-2"), 3).expect("d2");
    let (s4, _) = apply_event(&s3, &complete("act-2", b"r2"), 4).expect("c2");

    assert_eq!(s4.max_checkpointed_operation_id(), Some(1));
}

#[test]
fn activity_failed_exhausted_removes_from_in_flight() {
    let s0 = ProceduralActorState::new();
    let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
    let (s2, result) = apply_event(&s1, &fail("act-1", true), 2).expect("fail");
    assert!(matches!(
        result,
        ProceduralApplyResult::ActivityFailed { operation_id: 0 }
    ));
    assert!(!s2.in_flight.contains_key(&0));
    assert!(!s2.checkpoint_map.contains_key(&0));
}

#[test]
fn activity_failed_not_exhausted_stays_tracked() {
    let s0 = ProceduralActorState::new();
    let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
    let (s2, result) = apply_event(&s1, &fail("act-1", false), 2).expect("fail retry");
    assert!(matches!(result, ProceduralApplyResult::None));
    assert!(s2.in_flight.contains_key(&0));
}

#[test]
fn unknown_activity_id_on_complete_returns_error() {
    let s0 = ProceduralActorState::new();
    let result = apply_event(&s0, &complete("ghost", b"r"), 1);
    assert!(matches!(
        result,
        Err(ProceduralApplyError::UnknownActivityId(_))
    ));
}

#[test]
fn snapshot_taken_resets_events_since_snapshot() {
    let mut s = ProceduralActorState::new();
    s.events_since_snapshot = 50;
    let event = WorkflowEvent::SnapshotTaken {
        seq: 10,
        checksum: 0,
    };
    let (next, _) = apply_event(&s, &event, 11).expect("snapshot");
    assert_eq!(next.events_since_snapshot, 0);
}

#[test]
fn now_sampled_stores_checkpoint_with_encoded_ts() {
    let s0 = ProceduralActorState::new();
    let ts = chrono::DateTime::parse_from_rfc3339("2026-03-21T10:00:00Z")
        .expect("parse")
        .with_timezone(&chrono::Utc);
    let event = wtf_common::WorkflowEvent::NowSampled { operation_id: 0, ts };
    let (s1, _) = apply_event(&s0, &event, 1).expect("now sampled");
    assert!(s1.checkpoint_map.contains_key(&0), "checkpoint must be stored for op 0");
}

#[test]
fn random_sampled_stores_checkpoint_with_encoded_value() {
    let s0 = ProceduralActorState::new();
    let event = wtf_common::WorkflowEvent::RandomSampled { operation_id: 0, value: 42 };
    let (s1, _) = apply_event(&s0, &event, 1).expect("random sampled");
    assert!(s1.checkpoint_map.contains_key(&0), "checkpoint must be stored for op 0");
    let stored = u64::from_le_bytes(s1.checkpoint_map[&0].result.as_ref().try_into().expect("8 bytes"));
    assert_eq!(stored, 42);
}

// Compile-time guard: ProceduralNow and ProceduralRandom must exist as InstanceMsg variants.
// This test won't compile if the variants are missing.
#[test]
fn instance_msg_has_procedural_now_and_random_variants() {
    use crate::messages::InstanceMsg;
    // Just reference the variant names — the test body is intentionally empty.
    let _: fn(ractor::RpcReplyPort<chrono::DateTime<chrono::Utc>>) -> InstanceMsg =
        |reply| InstanceMsg::ProceduralNow { reply };
    let _: fn(ractor::RpcReplyPort<u64>) -> InstanceMsg =
        |reply| InstanceMsg::ProceduralRandom { reply };
}

// Compile-time guard: WorkflowContext must have now() and random_u64() methods.
#[allow(dead_code)]
fn _context_has_now_and_random_methods(ctx: &crate::procedural::WorkflowContext) {
    let _: std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<chrono::DateTime<chrono::Utc>>>>> =
        Box::pin(ctx.now());
    let _: std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<u64>>>> =
        Box::pin(ctx.random_u64());
}
