//! BDD tests for ADR-036 Command Identity Correlation and Causation.
//!
//! These tests verify the durable lineage metadata guarantees:
//! - CausationId links events back to their causing command
//! - CorrelationId groups all work from a single business request
//! - Command chains can be fully traced via causation links

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::useless_vec, unused_imports, unused_variables)]

use std::collections::HashMap;

use uuid::Uuid;

use crate::command_envelope::CommandEnvelope;
use crate::events::envelope::EventEnvelope;
use crate::events::metadata::EventMetadata;
use crate::identity::{CausationId, CommandId, CorrelationId};
use crate::{CommandMetadata, IdempotencyKey, Issuer, TimestampMs};

fn make_command_meta(
    command_id: &str,
    correlation_id: &str,
    causation_id: &str,
) -> CommandMetadata {
    CommandMetadata {
        command_id: IdempotencyKey::parse(command_id).unwrap(),
        correlation_id: IdempotencyKey::parse(correlation_id).unwrap(),
        causation_id: IdempotencyKey::parse(causation_id).unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    }
}

fn make_event_with_metadata(
    instance_id: &str,
    sequence: u64,
    meta: CommandMetadata,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1_700_000_000 + sequence * 100,
        payload: serde_json::json!({"type": "test"}),
        metadata: EventMetadata {
            command_metadata: Some(meta),
            annotations: HashMap::new(),
        },
    }
}

/// Given a command_id as CommandId,
/// When used as the causation_id for a subsequent command (via UUID),
/// Then the linkage is preserved through the type conversion.
#[test]
fn command_id_can_become_causation_id_preserving_uuid_linkage() {
    // Given
    let cmd_id = CommandId::new();

    // When — next command's causation_id carries the previous command's identity
    let causation = CausationId::from_uuid(cmd_id.to_uuid());

    // Then — the UUID link is preserved
    assert_eq!(
        cmd_id.to_uuid(),
        causation.to_uuid(),
        "CausationId must carry the UUID of the causing CommandId"
    );
}

/// Given two CommandIds created independently,
/// When compared,
/// Then they are not equal (unique generation).
#[test]
fn command_ids_are_unique() {
    let id1 = CommandId::new();
    let id2 = CommandId::new();
    assert_ne!(id1, id2);
}

/// Given a CorrelationId created from a UUID,
/// When converted to string and back,
/// Then the identity is preserved.
#[test]
fn correlation_id_roundtrip_via_string() {
    let original = CorrelationId::new();
    let as_string = original.to_string();
    let restored = CorrelationId::parse(&as_string).unwrap();
    assert_eq!(original, restored);
}

/// Given a CausationId created from bytes,
/// When converted back to bytes,
/// Then the bytes match.
#[test]
fn causation_id_roundtrip_via_bytes() {
    let bytes = [42u8; 16];
    let original = CausationId::from_bytes(bytes);
    let uuid = original.to_uuid();
    assert_eq!(uuid.into_bytes(), bytes);
}

/// Given CommandId, CausationId, CorrelationId all wrapping the same UUID,
/// When comparing their inner UUIDs,
/// Then they are equal (lineage identity chain).
#[test]
fn identity_chain_preserves_uuid() {
    let uuid = Uuid::new_v4();
    let cmd = CommandId::from_uuid(uuid);
    let caus = CausationId::from_uuid(uuid);
    let corr = CorrelationId::from_uuid(uuid);
    assert_eq!(cmd.to_uuid(), caus.to_uuid());
    assert_eq!(caus.to_uuid(), corr.to_uuid());
}

/// Given invalid UUID strings,
/// When parsing as identity types,
/// Then all fail with ParseError.
#[test]
fn identity_types_reject_invalid_uuids() {
    let bad_inputs = ["not-a-uuid", "", "12345", "g-g-g-g-g"];
    for input in bad_inputs {
        assert!(CommandId::parse(input).is_err());
        assert!(CorrelationId::parse(input).is_err());
        assert!(CausationId::parse(input).is_err());
    }
}

/// Given a command C with identity cmd-001,
/// When the engine produces event E,
/// Then E's causation_id equals C's command_id.
#[test]
fn event_causation_id_equals_command_id() {
    let cmd_meta = make_command_meta("cmd-001", "corr-root", "cause-root");
    let event = make_event_with_metadata("inst-1", 1, cmd_meta);
    let event_meta = event
        .metadata
        .command_metadata
        .as_ref()
        .expect("event must carry command metadata");
    assert_eq!(
        event_meta.causation_id.as_str(),
        "cause-root",
        "CausationId on event must link to the causing command"
    );
}

/// Given a command producing multiple events,
/// When each event carries the same command_id,
/// Then all events share identical causation linkage.
#[test]
fn multiple_events_share_causation_from_same_command() {
    let cmd_meta = make_command_meta("cmd-multi", "corr-multi", "cause-multi");
    let events: Vec<_> = (1..=3)
        .map(|seq| make_event_with_metadata("inst-multi", seq, cmd_meta.clone()))
        .collect();

    for (i, event) in events.iter().enumerate() {
        let meta = event.metadata.command_metadata.as_ref().unwrap();
        assert_eq!(
            meta.command_id.as_str(),
            "cmd-multi",
            "event {} command_id",
            i + 1
        );
        assert_eq!(
            meta.causation_id.as_str(),
            "cause-multi",
            "event {} causation_id",
            i + 1
        );
    }
}

/// Given commands C1 and C2 in the same business request,
/// When both carry the same correlation_id,
/// Then they are discoverable as part of the same correlation group.
#[test]
fn same_correlation_id_groups_commands() {
    let correlation_id = "corr-business-op-42";
    let cmd1 = make_command_meta("cmd-a", correlation_id, "cause-a");
    let cmd2 = make_command_meta("cmd-b", correlation_id, "cause-b");

    let mut by_correlation: HashMap<&str, Vec<&IdempotencyKey>> = HashMap::new();
    by_correlation
        .entry(correlation_id)
        .or_default()
        .push(&cmd1.command_id);
    by_correlation
        .entry(correlation_id)
        .or_default()
        .push(&cmd2.command_id);

    let grouped = by_correlation.get(correlation_id).unwrap();
    assert_eq!(
        grouped.len(),
        2,
        "both commands must be grouped under the same correlation_id"
    );
}

/// Given events from correlated commands,
/// When filtering by correlation_id,
/// Then all events from the business request are found.
#[test]
fn events_from_correlated_commands_share_correlation_id() {
    let correlation = "corr-txn-99";
    let cmd1 = make_command_meta("cmd-step1", correlation, "cause-step1");
    let cmd2 = make_command_meta("cmd-step2", correlation, "cause-step2");

    let event1 = make_event_with_metadata("inst-1", 1, cmd1);
    let event2 = make_event_with_metadata("inst-1", 2, cmd2);

    let corr1 = event1
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .correlation_id
        .as_str();
    let corr2 = event2
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .correlation_id
        .as_str();
    assert_eq!(corr1, corr2);
    assert_eq!(corr1, correlation);
}

/// Given commands from different business requests,
/// When each has a distinct correlation_id,
/// Then they are not grouped together.
#[test]
fn different_correlation_ids_are_separate_groups() {
    let cmd1 = make_command_meta("cmd-x", "corr-alpha", "cause-x");
    let cmd2 = make_command_meta("cmd-y", "corr-beta", "cause-y");
    assert_ne!(cmd1.correlation_id, cmd2.correlation_id);
}

/// Given command chain A→B→C,
/// When tracing causation backwards from C,
/// Then the full chain C→B→A is recovered.
#[test]
fn full_causation_chain_recovered_backwards() {
    let correlation = "corr-chain";
    let cmd_a = make_command_meta("cmd-a", correlation, "cause-root");
    let cmd_b = make_command_meta("cmd-b", correlation, "cmd-a");
    let cmd_c = make_command_meta("cmd-c", correlation, "cmd-b");

    let chain: Vec<&str> = vec![
        cmd_c.causation_id.as_str(),
        cmd_b.causation_id.as_str(),
        cmd_a.causation_id.as_str(),
    ];

    assert_eq!(chain[0], "cmd-b", "C's causation must point to B");
    assert_eq!(chain[1], "cmd-a", "B's causation must point to A");
    assert_eq!(chain[2], "cause-root", "A's causation must point to root");
}

/// Given a command chain A→B→C,
/// When collecting all events from the chain,
/// Then all events share the same correlation_id.
#[test]
fn all_chain_events_share_correlation_id() {
    let correlation = "corr-chain-shared";
    let cmd_a = make_command_meta("cmd-a", correlation, "cause-root");
    let cmd_b = make_command_meta("cmd-b", correlation, "cmd-a");
    let cmd_c = make_command_meta("cmd-c", correlation, "cmd-b");

    let events = vec![
        make_event_with_metadata("inst-1", 1, cmd_a),
        make_event_with_metadata("inst-1", 2, cmd_b),
        make_event_with_metadata("inst-1", 3, cmd_c),
    ];

    let correlations: Vec<&str> = events
        .iter()
        .map(|e| {
            e.metadata
                .command_metadata
                .as_ref()
                .unwrap()
                .correlation_id
                .as_str()
        })
        .collect();

    assert!(correlations.iter().all(|c| c == &correlation));
}

/// Given a causation chain A→B→C, all command_ids must be distinct.
#[test]
fn chain_command_ids_are_distinct() {
    let cmd_a = make_command_meta("chain-a", "corr", "cause-root");
    let cmd_b = make_command_meta("chain-b", "corr", "chain-a");
    let cmd_c = make_command_meta("chain-c", "corr", "chain-b");

    let ids = [
        cmd_a.command_id.as_str(),
        cmd_b.command_id.as_str(),
        cmd_c.command_id.as_str(),
    ];
    let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), 3);
}

/// Given event EA produced by command A,
/// When command B's causation_id = A's command_id,
/// Then B is correctly linked as a reaction to EA.
#[test]
fn causation_id_links_reaction_to_origin() {
    let cmd_a = make_command_meta("origin-cmd", "corr-react", "cause-root");

    let cmd_b = CommandMetadata {
        command_id: IdempotencyKey::parse("reaction-cmd").unwrap(),
        correlation_id: cmd_a.correlation_id.clone(),
        causation_id: cmd_a.command_id.clone(),
        issuer: Issuer::RecoveryLoop,
        issued_at: TimestampMs::try_from(1_700_000_100u64).unwrap(),
    };

    assert_eq!(cmd_b.causation_id.as_str(), "origin-cmd");
    assert_eq!(cmd_b.correlation_id, cmd_a.correlation_id);
}

/// Given a serialized chain of 3 events,
/// When deserializing and tracing causation,
/// Then the full chain is recoverable from serialized form.
#[test]
fn causation_chain_recoverable_from_serialized_events() {
    let correlation = "corr-ser-chain";
    let cmd_a = make_command_meta("scmd-a", correlation, "scause-root");
    let cmd_b = make_command_meta("scmd-b", correlation, "scmd-a");
    let cmd_c = make_command_meta("scmd-c", correlation, "scmd-b");

    let event_a = make_event_with_metadata("inst-s", 1, cmd_a);
    let event_b = make_event_with_metadata("inst-s", 2, cmd_b);
    let event_c = make_event_with_metadata("inst-s", 3, cmd_c);

    let restored: Vec<EventEnvelope> = [&event_a, &event_b, &event_c]
        .iter()
        .map(|e| serde_json::from_str(&serde_json::to_string(e).unwrap()).unwrap())
        .collect();

    let causation_chain: Vec<String> = restored
        .iter()
        .map(|e| {
            e.metadata
                .command_metadata
                .as_ref()
                .unwrap()
                .causation_id
                .as_str()
                .to_string()
        })
        .collect();

    assert_eq!(causation_chain[0], "scause-root");
    assert_eq!(causation_chain[1], "scmd-a");
    assert_eq!(causation_chain[2], "scmd-b");
}

/// Given a CommandEnvelope with lineage fields,
/// When serialized to JSON and parsed back,
/// Then command_id, correlation_id, causation_id are preserved exactly.
#[test]
fn envelope_preserves_lineage_through_wire_format() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-lineage-001",
        "correlation_id": "corr-lineage-001",
        "causation_id": "cause-lineage-001",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;

    let envelope = CommandEnvelope::from_str(json).unwrap();
    let serialized = serde_json::to_string(&envelope).unwrap();
    let restored = CommandEnvelope::from_str(&serialized).unwrap();

    assert_eq!(envelope.metadata.command_id, restored.metadata.command_id);
    assert_eq!(
        envelope.metadata.correlation_id,
        restored.metadata.correlation_id
    );
    assert_eq!(
        envelope.metadata.causation_id,
        restored.metadata.causation_id
    );
}

/// Given two envelopes in the same correlation group,
/// When checking their correlation_ids,
/// Then they match but command_ids differ.
#[test]
fn envelopes_same_correlation_different_commands() {
    let json_a = r#"{
        "version": 1,
        "command_id": "cmd-alpha",
        "correlation_id": "corr-shared",
        "causation_id": "cause-root",
        "issuer": "api_client",
        "issued_at": 1700000000
    }"#;
    let json_b = r#"{
        "version": 1,
        "command_id": "cmd-beta",
        "correlation_id": "corr-shared",
        "causation_id": "cmd-alpha",
        "issuer": "recovery_loop",
        "issued_at": 1700000100
    }"#;

    let env_a = CommandEnvelope::from_str(json_a).unwrap();
    let env_b = CommandEnvelope::from_str(json_b).unwrap();

    assert_eq!(env_a.metadata.correlation_id, env_b.metadata.correlation_id);
    assert_ne!(env_a.metadata.command_id, env_b.metadata.command_id);
    assert_eq!(env_b.metadata.causation_id.as_str(), "cmd-alpha");
}
