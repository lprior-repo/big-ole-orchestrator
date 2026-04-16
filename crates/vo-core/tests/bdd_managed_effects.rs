//! BDD tests for managed effect sink contracts (ADR-030).

use vo_core::{validate_effect_kinds, validate_workflow_sinks, KnownSinks, WorkflowSinkValidator};
use vo_types::effects::EffectKind;

// --- KnownSinks registry ---

#[test]
fn given_default_sinks_when_queried_then_contains_blob_http_sql() {
    let validator = WorkflowSinkValidator::new();
    assert!(validator.known_sinks().contains("blob"));
    assert!(validator.known_sinks().contains("http"));
    assert!(validator.known_sinks().contains("sql"));
    assert_eq!(validator.known_sinks().len(), 3);
}

#[test]
fn given_unknown_sink_identifier_when_checked_then_not_found() {
    let validator = WorkflowSinkValidator::new();
    assert!(!validator.known_sinks().contains("kafka"));
    assert!(!validator.known_sinks().contains(""));
}

// --- Single sink validation ---

#[test]
fn given_known_sink_when_validated_then_succeeds() {
    let validator = WorkflowSinkValidator::new();
    assert!(validator.validate_sink("blob").is_ok());
    assert!(validator.validate_sink("http").is_ok());
    assert!(validator.validate_sink("sql").is_ok());
}

#[test]
fn given_unknown_sink_when_validated_then_rejects_with_error() {
    let validator = WorkflowSinkValidator::new();
    let err = validator.validate_sink("kafka").unwrap_err();
    assert_eq!(err.error_code(), "unsupported_sink");
    assert_eq!(err.sink_identifier(), Some("kafka"));
    let msg = err.to_string();
    assert!(msg.contains("kafka") && msg.contains("blob"));
}

#[test]
fn given_empty_sink_when_validated_then_rejects_as_empty() {
    let validator = WorkflowSinkValidator::new();
    let err = validator.validate_sink("").unwrap_err();
    assert_eq!(err.error_code(), "empty_sink");
    assert_eq!(err.sink_identifier(), None);
}

// --- Batch sink validation ---

#[test]
fn given_all_known_sinks_when_batch_validated_then_succeeds() {
    assert!(validate_workflow_sinks(["blob", "http", "sql"]).is_ok());
}

#[test]
fn given_batch_with_unknown_sink_when_validated_then_returns_first_error() {
    assert!(validate_workflow_sinks(["blob", "kafka", "sql"]).is_err());
}

#[test]
fn given_batch_with_empty_sink_when_validated_then_rejects_immediately() {
    let err = validate_workflow_sinks(["blob", ""]).unwrap_err();
    assert_eq!(err.error_code(), "empty_sink");
}

// --- EffectKind-to-sink mapping ---

#[test]
fn given_http_call_effect_kind_when_validated_then_maps_to_http_sink() {
    assert!(validate_effect_kinds([EffectKind::HttpCall]).is_ok());
}

#[test]
fn given_sql_query_effect_kind_when_validated_then_maps_to_sql_sink() {
    assert!(validate_effect_kinds([EffectKind::SqlQuery]).is_ok());
}

#[test]
fn given_blob_write_effect_kind_when_validated_then_maps_to_blob_sink() {
    assert!(validate_effect_kinds([EffectKind::BlobWrite]).is_ok());
}

#[test]
fn given_all_effect_kinds_when_validated_then_all_succeed() {
    assert!(validate_effect_kinds([
        EffectKind::HttpCall,
        EffectKind::SqlQuery,
        EffectKind::BlobWrite,
    ])
    .is_ok());
}

// --- Custom sink registry ---

#[test]
fn given_custom_sinks_when_validator_created_then_accepts_only_custom() {
    let custom = KnownSinks::new(["kafka", "redis"]);
    let validator = WorkflowSinkValidator::with_sinks(custom);
    assert!(validator.validate_sink("kafka").is_ok());
    assert!(validator.validate_sink("redis").is_ok());
    assert!(validator.validate_sink("blob").is_err());
}

#[test]
fn given_empty_custom_registry_when_validated_then_rejects_all_sinks() {
    let empty = KnownSinks::new([] as [&str; 0]);
    let validator = WorkflowSinkValidator::with_sinks(empty);
    assert!(validator.validate_sink("blob").is_err());
    assert!(validator.validate_sink("http").is_err());
}
