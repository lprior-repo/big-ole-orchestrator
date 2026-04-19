//! Handler-level unit tests for vo-api.
//!
//! Tests the pure logic parts of handlers: input validation, error mapping,
//! helper functions, and type construction that don't require a live actor.

use vo_api::handlers::helpers::{paradigm_to_str, parse_paradigm, phase_to_str, split_path_id};
use vo_api::types::errors::{
    InvariantViolation, ParseError, ValidationError, WorkloadRejectionError,
};
use vo_api::types::v3::{
    ApiError, EffectJournalEntry, EffectJournalResponse, EffectSemantics, HistoryEntry,
    HistoryResponse, SearchRequest, SearchResponse, SearchResultEntry, TimelineEntry,
    TimelineResponse, V3SignalRequest, V3StartRequest, V3StartResponse, V3StatusResponse,
    WorkflowVersionResponse,
};

// ---------------------------------------------------------------------------
// parse_paradigm — positive cases (previously untested)
// ---------------------------------------------------------------------------

#[test]
fn parse_paradigm_fsm() {
    assert!(matches!(parse_paradigm("fsm"), Some(_)));
}

#[test]
fn parse_paradigm_dag() {
    assert!(matches!(parse_paradigm("dag"), Some(_)));
}

#[test]
fn parse_paradigm_procedural() {
    assert!(matches!(parse_paradigm("procedural"), Some(_)));
}

#[test]
fn parse_paradigm_rejects_empty() {
    assert!(parse_paradigm("").is_none());
}

#[test]
fn parse_paradigm_rejects_uppercase() {
    assert!(parse_paradigm("FSM").is_none());
    assert!(parse_paradigm("DAG").is_none());
}

#[test]
fn parse_paradigm_rejects_unknown() {
    assert!(parse_paradigm("quantum").is_none());
    assert!(parse_paradigm("random").is_none());
}

// ---------------------------------------------------------------------------
// paradigm_to_str — previously untested
// ---------------------------------------------------------------------------

#[test]
fn paradigm_to_str_fsm() {
    let p = parse_paradigm("fsm").unwrap();
    assert_eq!(paradigm_to_str(p), "fsm");
}

#[test]
fn paradigm_to_str_dag() {
    let p = parse_paradigm("dag").unwrap();
    assert_eq!(paradigm_to_str(p), "dag");
}

#[test]
fn paradigm_to_str_procedural() {
    let p = parse_paradigm("procedural").unwrap();
    assert_eq!(paradigm_to_str(p), "procedural");
}

// ---------------------------------------------------------------------------
// phase_to_str — previously untested
// ---------------------------------------------------------------------------

#[test]
fn phase_to_str_replay() {
    use vo_actor::InstancePhaseView;
    assert_eq!(phase_to_str(InstancePhaseView::Replay), "replay");
}

#[test]
fn phase_to_str_live() {
    use vo_actor::InstancePhaseView;
    assert_eq!(phase_to_str(InstancePhaseView::Live), "live");
}

// ---------------------------------------------------------------------------
// split_path_id — edge cases
// ---------------------------------------------------------------------------

#[test]
fn split_path_id_single_char_namespace() {
    let result = split_path_id("a/01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert!(result.is_some());
    let (ns, _) = result.unwrap();
    assert_eq!(ns, "a");
}

#[test]
fn split_path_id_long_namespace() {
    let result = split_path_id("my-very-long-namespace-name/01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert!(result.is_some());
    let (ns, _) = result.unwrap();
    assert_eq!(ns, "my-very-long-namespace-name");
}

#[test]
fn split_path_id_extra_content_after_slash_rejected() {
    // InstanceId::parse rejects strings with extra slashes
    let result = split_path_id("ns/sub/01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert!(result.is_none());
}

#[test]
fn split_path_id_slash_only() {
    let result = split_path_id("/");
    // After slash is empty — InstanceId::parse("") should fail
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// SearchRequest / SearchResponse / SearchResultEntry — previously untested
// ---------------------------------------------------------------------------

#[test]
fn search_request_serializes_with_query() {
    let req = SearchRequest {
        query: "workflow".to_string(),
        limit: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("workflow"));
    assert!(json.contains("null")); // limit: None serializes as null
}

#[test]
fn search_request_serializes_with_limit() {
    let req = SearchRequest {
        query: "test".to_string(),
        limit: Some(50),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("50"));
}

#[test]
fn search_request_roundtrip() {
    let req = SearchRequest {
        query: "payment".to_string(),
        limit: Some(25),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: SearchRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.query, "payment");
    assert_eq!(back.limit, Some(25));
}

#[test]
fn search_request_deserializes_without_limit() {
    let json = r#"{"query":"hello"}"#;
    let req: SearchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.query, "hello");
    assert!(req.limit.is_none());
}

#[test]
fn search_result_entry_serializes() {
    let entry = SearchResultEntry {
        workspace_id: "ws-1".to_string(),
        score: 0.95,
        matched_terms: vec!["workflow".to_string()],
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("ws-1"));
    assert!(json.contains("0.95"));
    assert!(json.contains("workflow"));
}

#[test]
fn search_result_entry_roundtrip() {
    let entry = SearchResultEntry {
        workspace_id: "ws-2".to_string(),
        score: 1.0,
        matched_terms: vec!["a".to_string(), "b".to_string()],
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: SearchResultEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.workspace_id, "ws-2");
    assert_eq!(back.matched_terms.len(), 2);
}

#[test]
fn search_response_roundtrip() {
    let resp = SearchResponse {
        query: "test".to_string(),
        results: vec![SearchResultEntry {
            workspace_id: "ws-1".to_string(),
            score: 0.5,
            matched_terms: vec!["test".to_string()],
        }],
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: SearchResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(back.query, "test");
    assert_eq!(back.results.len(), 1);
}

#[test]
fn search_response_empty_results() {
    let resp = SearchResponse {
        query: "nothing".to_string(),
        results: vec![],
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: SearchResponse = serde_json::from_str(&json).unwrap();
    assert!(back.results.is_empty());
}

// ---------------------------------------------------------------------------
// UnquarantineRequest / UnquarantineResponse — previously untested
// ---------------------------------------------------------------------------

#[test]
fn unquarantine_request_deserializes() {
    let json = r#"{"operator":"admin"}"#;
    let req: vo_api::handlers::workflow_lifecycle::UnquarantineRequest =
        serde_json::from_str(json).unwrap();
    assert_eq!(req.operator, "admin");
}

#[test]
fn unquarantine_response_serializes() {
    let resp = vo_api::handlers::workflow_lifecycle::UnquarantineResponse {
        workflow_name: "my-workflow".to_string(),
        previous_status: "quarantined".to_string(),
        new_status: "active".to_string(),
        failures_cleared: 3,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("my-workflow"));
    assert!(json.contains("quarantined"));
    assert!(json.contains("active"));
    assert!(json.contains("3"));
}

// ---------------------------------------------------------------------------
// WorkflowStatusResponse — previously untested
// ---------------------------------------------------------------------------

#[test]
fn workflow_status_response_serializes() {
    let resp = vo_api::handlers::workflow_status::WorkflowStatusResponse {
        instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        namespace: "payments".to_string(),
        workflow_type: "charge".to_string(),
        paradigm: "fsm".to_string(),
        phase: "live".to_string(),
        events_applied: 42,
        registration_status: Some("registered".to_string()),
        is_quarantined: false,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
    assert!(json.contains("registered"));
    assert!(json.contains("false"));
}

// ---------------------------------------------------------------------------
// Error mapping consistency — validate all error paths produce correct status
// ---------------------------------------------------------------------------

#[test]
fn workload_rejection_budget_exhausted_is_429() {
    let err = WorkloadRejectionError::BudgetExhausted {
        class: "standard".into(),
        requested: 5,
        available: 0,
    };
    assert_eq!(err.status_code(), 429);
    assert_eq!(err.error_code(), "budget_exhausted");
}

#[test]
fn workload_rejection_cap_exceeded_is_429() {
    let err = WorkloadRejectionError::WorkflowCapExceeded {
        class: "bulk".into(),
        workflow_id: "wf-123".into(),
    };
    assert_eq!(err.status_code(), 429);
    assert_eq!(err.error_code(), "workflow_cap_exceeded");
}

#[test]
fn workload_rejection_global_limit_is_503() {
    let err = WorkloadRejectionError::GlobalConcurrencyLimit {
        current: 100,
        max: 100,
    };
    assert_eq!(err.status_code(), 503);
    assert_eq!(err.error_code(), "global_concurrency_limit");
}

// ---------------------------------------------------------------------------
// ApiError — ensure error/message are never empty
// ---------------------------------------------------------------------------

#[test]
fn api_error_has_both_fields() {
    let err = ApiError::new("code", "msg");
    assert_eq!(err.error, "code");
    assert_eq!(err.message, "msg");
}

#[test]
fn api_error_serializes_to_standard_envelope() {
    let err = ApiError::new("not_found", "instance missing");
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains(r#""error":"not_found""#));
    assert!(json.contains(r#""message":"instance missing""#));
}

// ---------------------------------------------------------------------------
// V3StartRequest — input validation edge cases
// ---------------------------------------------------------------------------

#[test]
fn v3_start_request_empty_namespace_deserializes() {
    let json = r#"{"namespace":"","workflow_type":"wf","paradigm":"fsm","input":{}}"#;
    let req: V3StartRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.namespace, "");
}

#[test]
fn v3_start_request_null_input() {
    let json = r#"{"namespace":"ns","workflow_type":"wf","paradigm":"fsm","input":null}"#;
    let req: V3StartRequest = serde_json::from_str(json).unwrap();
    assert!(req.input.is_null());
}

#[test]
fn v3_start_request_large_input() {
    let big = "x".repeat(100_000);
    let json = format!(
        r#"{{"namespace":"ns","workflow_type":"wf","paradigm":"fsm","input":{{"data":"{big}"}}}}"#
    );
    let req: V3StartRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(req.input["data"].as_str().unwrap().len(), 100_000);
}

// ---------------------------------------------------------------------------
// V3SignalRequest — edge cases
// ---------------------------------------------------------------------------

#[test]
fn v3_signal_request_empty_name() {
    let req = V3SignalRequest {
        signal_name: "".to_string(),
        payload: serde_json::json!({}),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: V3SignalRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.signal_name, "");
}

#[test]
fn v3_signal_request_null_payload() {
    let req = V3SignalRequest {
        signal_name: "test".to_string(),
        payload: serde_json::Value::Null,
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: V3SignalRequest = serde_json::from_str(&json).unwrap();
    assert!(back.payload.is_null());
}

// ---------------------------------------------------------------------------
// EffectJournalEntry — semantics edge cases
// ---------------------------------------------------------------------------

#[test]
fn effect_semantics_unsafe_roundtrip() {
    let entry = EffectJournalEntry {
        sequence: 1,
        timestamp_ms: 0,
        event_type: "test".to_string(),
        semantics: EffectSemantics::Unsafe,
        payload: serde_json::json!({}),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: EffectJournalEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.semantics, EffectSemantics::Unsafe);
}

#[test]
fn effect_journal_response_empty_entries() {
    let resp = EffectJournalResponse {
        instance_id: "id".to_string(),
        entries: vec![],
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: EffectJournalResponse = serde_json::from_str(&json).unwrap();
    assert!(back.entries.is_empty());
}

// ---------------------------------------------------------------------------
// HistoryResponse — empty entries edge case
// ---------------------------------------------------------------------------

#[test]
fn history_response_empty_entries() {
    let resp = HistoryResponse {
        instance_id: "id".to_string(),
        entries: vec![],
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: HistoryResponse = serde_json::from_str(&json).unwrap();
    assert!(back.entries.is_empty());
}

// ---------------------------------------------------------------------------
// TimelineResponse — empty entries edge case
// ---------------------------------------------------------------------------

#[test]
fn timeline_response_empty_entries() {
    let resp = TimelineResponse {
        instance_id: "id".to_string(),
        entries: vec![],
        total_replayed: 0,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: TimelineResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(back.total_replayed, 0);
}
