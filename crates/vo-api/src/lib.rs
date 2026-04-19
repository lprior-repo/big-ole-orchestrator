//! HTTP API for vo-engine.
//!
//! This crate provides the REST API for the Veloxide workflow engine,
//! including endpoints for workflow management, step execution, and
//! system health monitoring.
//!
//! # API Version
//!
//! The current API version is v3, providing:
//! - [`types::v3`] - Request/response types for v3 endpoints
//! - [`types::errors`] - Standard API error types
//! - [`handlers::query`] - Query handlers for workflow status
//!
//! # Endpoint Overview
//!
//! - `POST /api/v1/workflows` - Start a new workflow instance
//! - `GET /api/v1/workflows/:id` - Get workflow status
//! - `GET /api/v1/workflows/:id/timeline` - Get timeline events
//! - `GET /api/v1/workflows/:id/history` - Get step execution history
//! - `GET /api/v1/workflows/:id/effect-journal` - Get effect journal
//! - `GET /api/v1/workflows/:id/version` - Get schema version info
//! - `GET /api/v1/search` - Full-text search across workspaces
//! - `POST /api/v1/workflows/:id/signals` - Send a signal to a workflow
//!
//! # Modules
//!
//! - [`types`] - Request/response types for the API
//! - [`handlers`] - HTTP request handlers (query endpoints active; workflow, signal, events, sse pending V2 actor migration)

pub mod handlers;
pub mod router;
pub mod types;

#[cfg(test)]
mod lib_tests {
    #[test]
    fn crate_root_test_smoke() {
        assert!(true);
    }

    #[test]
    fn timeline_entry_serializes_with_all_fields() {
        let entry = crate::types::v3::TimelineEntry {
            sequence: 1,
            timestamp_ms: 1_714_000_000_000,
            event_type: "workflow_started".to_string(),
            payload: serde_json::json!({"workflow_id": "wf-1"}),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""sequence":1"#));
        assert!(json.contains(r#""event_type":"workflow_started""#));
        assert!(json.contains(r#""timestamp_ms":1714000000000"#));
    }

    #[test]
    fn timeline_entry_deserializes_roundtrip() {
        let json = r#"{"sequence":5,"timestamp_ms":1000,"event_type":"step_completed","payload":{"step":"a"}}"#;
        let entry: crate::types::v3::TimelineEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.sequence, 5);
        assert_eq!(entry.event_type, "step_completed");
        assert_eq!(entry.payload["step"], "a");
    }

    #[test]
    fn timeline_response_serializes() {
        let resp = crate::types::v3::TimelineResponse {
            instance_id: "inst-1".to_string(),
            entries: vec![],
            total_replayed: 0,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""instance_id":"inst-1""#));
        assert!(json.contains(r#""total_replayed":0"#));
    }

    // --- HistoryEntry tests ---

#[test]
fn history_entry_omits_none_fields() {
    let entry = crate::types::v3::HistoryEntry {
        sequence: 1,
        timestamp_ms: 0,
        event_type: "workflow_started".to_string(),
        step_id: None,
        error: None,
        output: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("step_id"));
    assert!(!json.contains("error"));
    assert!(!json.contains("output"));
}

    #[test]
    fn effect_semantics_exact_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&crate::types::v3::EffectSemantics::Exact).unwrap(),
            r#""exact""#
        );
    }

    #[test]
    fn effect_semantics_unsafe_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&crate::types::v3::EffectSemantics::Unsafe).unwrap(),
            r#""unsafe""#
        );
    }

    #[test]
    fn effect_semantics_roundtrip() {
        for variant in [
            crate::types::v3::EffectSemantics::Exact,
            crate::types::v3::EffectSemantics::Unsafe,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: crate::types::v3::EffectSemantics = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, variant);
        }
    }

    // --- EffectJournalEntry tests ---

    #[test]
    fn effect_journal_entry_serializes_with_semantics() {
        let entry = crate::types::v3::EffectJournalEntry {
            sequence: 2,
            timestamp_ms: 1000,
            event_type: "step_completed".to_string(),
            semantics: crate::types::v3::EffectSemantics::Exact,
            payload: serde_json::json!({}),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""semantics":"exact""#));
    }

    // --- WorkflowVersionResponse tests ---

    #[test]
    fn workflow_version_response_serializes() {
        let resp = crate::types::v3::WorkflowVersionResponse {
            instance_id: "inst-4".to_string(),
            schema_version: 1,
            event_count: 42,
            last_sequence: Some(42),
            last_timestamp_ms: Some(1714000000000),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""schema_version":1"#));
        assert!(json.contains(r#""event_count":42"#));
        assert!(json.contains(r#""last_sequence":42"#));
    }

    #[test]
    fn workflow_version_response_handles_empty_stream() {
        let resp = crate::types::v3::WorkflowVersionResponse {
            instance_id: "inst-5".to_string(),
            schema_version: 1,
            event_count: 0,
            last_sequence: None,
            last_timestamp_ms: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""last_sequence":null"#));
        assert!(json.contains(r#""last_timestamp_ms":null"#));
    }
}
