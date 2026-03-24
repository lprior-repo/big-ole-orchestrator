#![allow(clippy::expect_used)]
//! `append_event()` — the ONLY function that publishes `WorkflowEvents` to NATS `JetStream`.
//!
//! Architecture invariant (ADR-015): no code outside this module may call
//! `jetstream.publish()` directly. All state transitions go through `append_event`.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use async_nats::jetstream::Context;
use bytes::Bytes;
use wtf_common::{InstanceId, NamespaceId, WorkflowEvent, WtfError};

const PUBLISH_ACK_TIMEOUT: Duration = Duration::from_secs(5);

/// Append a `WorkflowEvent` to the `JetStream` log for this instance.
///
/// The caller MUST await Ok(seq) before executing any side effect (ADR-015).
/// Subject: wtf.log.<namespace>.<`instance_id`>
///
/// # Errors
/// - `WtfError::NatsPublish` on publish failure.
/// - `WtfError::NatsTimeout` if `PublishAck` not received within 5s.
pub async fn append_event(
    js: &Context,
    namespace: &NamespaceId,
    instance_id: &InstanceId,
    event: &WorkflowEvent,
) -> Result<u64, WtfError> {
    let subject = build_subject(namespace, instance_id);
    let payload = serialize_event(event)?;

    let ack_future = js
        .publish(subject, payload)
        .await
        .map_err(|e| WtfError::nats_publish(format!("publish failed: {e}")))?;

    let ack = tokio::time::timeout(PUBLISH_ACK_TIMEOUT, ack_future)
        .await
        .map_err(|_| {
            WtfError::nats_timeout("await PublishAck", PUBLISH_ACK_TIMEOUT.as_millis() as u64)
        })?
        .map_err(|e| WtfError::nats_publish(format!("ack error: {e}")))?;

    Ok(ack.sequence)
}

/// Build the NATS subject: wtf.log.<namespace>.<`instance_id`>
#[must_use]
pub fn build_subject(namespace: &NamespaceId, instance_id: &InstanceId) -> String {
    format!("wtf.log.{}.{}", namespace.as_str(), instance_id.as_str())
}

fn serialize_event(event: &WorkflowEvent) -> Result<Bytes, WtfError> {
    rmp_serde::to_vec_named(event)
        .map(Bytes::from)
        .map_err(|e| WtfError::nats_publish(format!("serialize event: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_subject_format() {
        let ns = NamespaceId::new("payments");
        let id = InstanceId::new("01ARZ");
        assert_eq!(build_subject(&ns, &id), "wtf.log.payments.01ARZ");
    }

    #[test]
    fn build_subject_different_namespace() {
        let ns = NamespaceId::new("onboarding");
        let id = InstanceId::new("01BQA");
        assert_eq!(build_subject(&ns, &id), "wtf.log.onboarding.01BQA");
    }

    #[test]
    fn serialize_event_produces_non_empty_bytes() {
        let event = WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        let bytes = serialize_event(&event).expect("serialize");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn serialize_event_roundtrips_activity_completed() {
        let event = WorkflowEvent::ActivityCompleted {
            activity_id: "act-1".into(),
            result: bytes::Bytes::from_static(b"ok"),
            duration_ms: 42,
        };
        let bytes = serialize_event(&event).expect("serialize");
        let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
        assert_eq!(event, decoded);
    }

    #[test]
    fn serialize_event_roundtrips_transition_applied() {
        use wtf_common::EffectDeclaration;
        let event = WorkflowEvent::TransitionApplied {
            from_state: "Pending".into(),
            event_name: "Authorize".into(),
            to_state: "Authorized".into(),
            effects: vec![EffectDeclaration {
                effect_type: "CallAuthorizationService".into(),
                payload: bytes::Bytes::from_static(b"{}"),
            }],
        };
        let bytes = serialize_event(&event).expect("serialize");
        let decoded = WorkflowEvent::from_msgpack(&bytes).expect("decode");
        assert_eq!(event, decoded);
    }

    // append_event() requires a live NATS server — covered by integration tests (wtf-rakc).
}
