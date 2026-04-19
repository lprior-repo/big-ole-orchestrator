use serde_json::json;
use vo_api::types::errors::*;
use vo_api::types::helpers::*;
use vo_api::types::names::{RetryAfterSeconds, Timestamp};
use vo_api::types::v1::{StartWorkflowResponse, WorkflowStatusValue};
use vo_api::types::v3::*;

mod v3_start_request_flow {
    use super::*;

    #[test]
    fn minimal_start_request_serializes_correctly() {
        let req = V3StartRequest {
            namespace: "payments".to_string(),
            workflow_type: "checkout".to_string(),
            paradigm: "fsm".to_string(),
            input: json!({"order_id": "ord_123"}),
            instance_id: None,
            dedupe_key: None,
        };
        let json_str = serde_json::to_string(&req).unwrap();
        assert!(json_str.contains(r#""namespace":"payments""#));
        assert!(json_str.contains(r#""workflow_type":"checkout""#));
        assert!(json_str.contains(r#""paradigm":"fsm""#));
        assert!(!json_str.contains("instance_id"));
        assert!(!json_str.contains("dedupe_key"));
    }

    #[test]
    fn full_start_request_with_all_fields() {
        let req = V3StartRequest {
            namespace: "orders".to_string(),
            workflow_type: "process_order".to_string(),
            paradigm: "dag".to_string(),
            input: json!({"items": ["a", "b"], "priority": 1}),
            instance_id: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
            dedupe_key: Some("dedupe-abc-123".to_string()),
        };
        let json_str = serde_json::to_string(&req).unwrap();
        assert!(json_str.contains(r#""instance_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV""#));
        assert!(json_str.contains(r#""dedupe_key":"dedupe-abc-123""#));
    }

    #[test]
    fn start_request_deserializes_from_json() {
        let json_val = json!({
            "namespace": "inventory",
            "workflow_type": "stock_check",
            "paradigm": "procedural",
            "input": {"sku": "ABC123"}
        });
        let req: V3StartRequest = serde_json::from_value(json_val).unwrap();
        assert_eq!(req.namespace, "inventory");
        assert_eq!(req.workflow_type, "stock_check");
        assert_eq!(req.paradigm, "procedural");
        assert_eq!(req.input["sku"], "ABC123");
    }

    #[test]
    fn start_request_with_instance_id_roundtrip() {
        let req = V3StartRequest {
            namespace: "test".to_string(),
            workflow_type: "wf".to_string(),
            paradigm: "fsm".to_string(),
            input: json!({}),
            instance_id: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
            dedupe_key: None,
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: V3StartRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req.instance_id, deserialized.instance_id);
    }
}

mod v3_signal_request_flow {
    use super::*;

    #[test]
    fn signal_request_serializes() {
        let req = V3SignalRequest {
            signal_name: "payment_approved".to_string(),
            payload: json!({"amount": 100, "currency": "USD"}),
        };
        let json_str = serde_json::to_string(&req).unwrap();
        assert!(json_str.contains(r#""signal_name":"payment_approved""#));
        assert!(json_str.contains(r#""amount":100"#));
    }

    #[test]
    fn signal_request_roundtrip() {
        let req = V3SignalRequest {
            signal_name: "cancel".to_string(),
            payload: json!({"reason": "user_requested", "cancelled_at": "2024-01-15T10:30:00Z"}),
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: V3SignalRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req.signal_name, deserialized.signal_name);
        assert_eq!(req.payload, deserialized.payload);
    }

    #[test]
    fn signal_request_with_complex_payload() {
        let payload = json!({
            "nested": {
                "deep": {
                    "value": [1, 2, 3]
                }
            },
            "null_field": null,
            "bool_field": true
        });
        let req = V3SignalRequest {
            signal_name: "complex_signal".to_string(),
            payload,
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: V3SignalRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.payload["nested"]["deep"]["value"],
            json!([1, 2, 3])
        );
    }
}

mod v3_response_flow {
    use super::*;

    #[test]
    fn start_response_serializes() {
        let resp = V3StartResponse {
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            namespace: "payments".to_string(),
            workflow_type: "checkout".to_string(),
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains(r#""instance_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV""#));
        assert!(json_str.contains(r#""namespace":"payments""#));
        assert!(json_str.contains(r#""workflow_type":"checkout""#));
    }

    #[test]
    fn start_response_roundtrip() {
        let resp = V3StartResponse {
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            namespace: "orders".to_string(),
            workflow_type: "process".to_string(),
        };
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: V3StartResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(resp.instance_id, deserialized.instance_id);
        assert_eq!(resp.namespace, deserialized.namespace);
        assert_eq!(resp.workflow_type, deserialized.workflow_type);
    }

    #[test]
    fn status_response_serializes() {
        let resp = V3StatusResponse {
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            namespace: "payments".to_string(),
            workflow_type: "checkout".to_string(),
            paradigm: "fsm".to_string(),
            phase: "live".to_string(),
            events_applied: 42,
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains(r#""events_applied":42"#));
        assert!(json_str.contains(r#""phase":"live""#));
    }

    #[test]
    fn status_response_replay_phase() {
        let resp = V3StatusResponse {
            instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            namespace: "payments".to_string(),
            workflow_type: "checkout".to_string(),
            paradigm: "dag".to_string(),
            phase: "replay".to_string(),
            events_applied: 0,
        };
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: V3StatusResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(resp.phase, deserialized.phase);
    }
}

mod timeline_entry_flow {
    use super::*;

    #[test]
    fn timeline_entry_serializes_all_fields() {
        let entry = TimelineEntry {
            sequence: 1,
            timestamp_ms: 1_714_000_000_000,
            event_type: "workflow_started".to_string(),
            payload: json!({"workflow_id": "wf-1"}),
        };
        let json_str = serde_json::to_string(&entry).unwrap();
        assert!(json_str.contains(r#""sequence":1"#));
        assert!(json_str.contains(r#""timestamp_ms":1714000000000"#));
        assert!(json_str.contains(r#""event_type":"workflow_started""#));
    }

    #[test]
    fn timeline_entry_roundtrip() {
        let entry = TimelineEntry {
            sequence: 5,
            timestamp_ms: 1714000000000,
            event_type: "step_completed".to_string(),
            payload: json!({"step": "a", "result": "ok"}),
        };
        let serialized = serde_json::to_string(&entry).unwrap();
        let deserialized: TimelineEntry = serde_json::from_str(&serialized).unwrap();
        assert_eq!(entry.sequence, deserialized.sequence);
        assert_eq!(entry.event_type, deserialized.event_type);
    }

    #[test]
    fn timeline_response_serializes() {
        let resp = TimelineResponse {
            instance_id: "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            entries: vec![
                TimelineEntry {
                    sequence: 1,
                    timestamp_ms: 1000,
                    event_type: "started".to_string(),
                    payload: json!({}),
                },
                TimelineEntry {
                    sequence: 2,
                    timestamp_ms: 2000,
                    event_type: "completed".to_string(),
                    payload: json!({}),
                },
            ],
            total_replayed: 2,
            replay_error_count: 0,
            truncated: false,
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains(r#""total_replayed":2"#));
        assert!(json_str.contains(r#""entries":"#));
    }

    #[test]
    fn timeline_response_empty_entries() {
        let resp = TimelineResponse {
            instance_id: "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            entries: vec![],
            total_replayed: 0,
            replay_error_count: 0,
            truncated: false,
        };
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: TimelineResponse = serde_json::from_str(&serialized).unwrap();
        assert!(deserialized.entries.is_empty());
        assert_eq!(deserialized.total_replayed, 0);
    }
}

mod history_entry_flow {
    use super::*;

    #[test]
    fn history_entry_with_all_fields() {
        let entry = HistoryEntry {
            sequence: 3,
            timestamp_ms: 5000,
            event_type: "step_completed".to_string(),
            step_id: Some("build".to_string()),
            error: None,
            output: Some(json!({"result": "ok"})),
        };
        let json_str = serde_json::to_string(&entry).unwrap();
        assert!(json_str.contains(r#""step_id":"build""#));
        assert!(json_str.contains("output"));
        assert!(!json_str.contains("error"));
    }

    #[test]
    fn history_entry_error_field() {
        let entry = HistoryEntry {
            sequence: 4,
            timestamp_ms: 6000,
            event_type: "step_failed".to_string(),
            step_id: Some("deploy".to_string()),
            error: Some("timeout".to_string()),
            output: None,
        };
        let json_str = serde_json::to_string(&entry).unwrap();
        assert!(json_str.contains(r#""error":"timeout""#));
        assert!(!json_str.contains("output"));
    }

    #[test]
    fn history_entry_omits_none_fields() {
        let entry = HistoryEntry {
            sequence: 1,
            timestamp_ms: 0,
            event_type: "workflow_started".to_string(),
            step_id: None,
            error: None,
            output: None,
        };
        let json_str = serde_json::to_string(&entry).unwrap();
        assert!(!json_str.contains("step_id"));
        assert!(!json_str.contains("error"));
        assert!(!json_str.contains("output"));
    }

    #[test]
    fn history_response_roundtrip() {
        let resp = HistoryResponse {
            instance_id: "orders/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            entries: vec![HistoryEntry {
                sequence: 1,
                timestamp_ms: 1000,
                event_type: "started".to_string(),
                step_id: None,
                error: None,
                output: None,
            }],
            replay_error_count: 0,
            truncated: false,
        };
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: HistoryResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(resp.instance_id, deserialized.instance_id);
        assert_eq!(resp.entries.len(), deserialized.entries.len());
    }
}

mod effect_journal_flow {
    use super::*;

    #[test]
    fn effect_semantics_exact_serializes() {
        let semantics = EffectSemantics::Exact;
        let json_str = serde_json::to_string(&semantics).unwrap();
        assert_eq!(json_str, r#""exact""#);
    }

    #[test]
    fn effect_semantics_unsafe_serializes() {
        let semantics = EffectSemantics::Unsafe;
        let json_str = serde_json::to_string(&semantics).unwrap();
        assert_eq!(json_str, r#""unsafe""#);
    }

    #[test]
    fn effect_semantics_roundtrip() {
        for semantics in [EffectSemantics::Exact, EffectSemantics::Unsafe] {
            let serialized = serde_json::to_string(&semantics).unwrap();
            let deserialized: EffectSemantics = serde_json::from_str(&serialized).unwrap();
            assert_eq!(semantics, deserialized);
        }
    }

    #[test]
    fn effect_journal_entry_roundtrip() {
        let entry = EffectJournalEntry {
            sequence: 2,
            timestamp_ms: 1000,
            event_type: "step_completed".to_string(),
            semantics: EffectSemantics::Exact,
            payload: json!({"effect": "send_email"}),
        };
        let serialized = serde_json::to_string(&entry).unwrap();
        let deserialized: EffectJournalEntry = serde_json::from_str(&serialized).unwrap();
        assert_eq!(entry.sequence, deserialized.sequence);
        assert_eq!(entry.semantics, deserialized.semantics);
    }

    #[test]
    fn effect_journal_response_roundtrip() {
        let resp = EffectJournalResponse {
            instance_id: "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            entries: vec![EffectJournalEntry {
                sequence: 1,
                timestamp_ms: 1000,
                event_type: "effect_executed".to_string(),
                semantics: EffectSemantics::Exact,
                payload: json!({}),
            }],
            replay_error_count: 0,
            truncated: false,
        };
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: EffectJournalResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(resp.instance_id, deserialized.instance_id);
        assert_eq!(resp.entries.len(), 1);
    }
}

mod workflow_version_response_flow {
    use super::*;

    #[test]
    fn workflow_version_response_full() {
        let resp = WorkflowVersionResponse {
            instance_id: "inst-4".to_string(),
            schema_version: 1,
            event_count: 42,
            last_sequence: Some(42),
            last_timestamp_ms: Some(1714000000000),
            replay_error_count: 0,
            truncated: false,
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains(r#""schema_version":1"#));
        assert!(json_str.contains(r#""event_count":42"#));
        assert!(json_str.contains(r#""last_sequence":42"#));
    }

    #[test]
    fn workflow_version_response_empty_stream() {
        let resp = WorkflowVersionResponse {
            instance_id: "inst-new".to_string(),
            schema_version: 1,
            event_count: 0,
            last_sequence: None,
            last_timestamp_ms: None,
            replay_error_count: 0,
            truncated: false,
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains(r#""last_sequence":null"#));
        assert!(json_str.contains(r#""last_timestamp_ms":null"#));
        assert!(json_str.contains(r#""event_count":0"#));
    }

    #[test]
    fn workflow_version_response_roundtrip() {
        let resp = WorkflowVersionResponse {
            instance_id: "orders/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            schema_version: 1,
            event_count: 100,
            last_sequence: Some(100),
            last_timestamp_ms: Some(1714000000000),
            replay_error_count: 0,
            truncated: false,
        };
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: WorkflowVersionResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(resp.schema_version, deserialized.schema_version);
        assert_eq!(resp.event_count, deserialized.event_count);
    }
}

mod api_error_flow {
    use super::*;

    #[test]
    fn api_error_new() {
        let err = ApiError::new("not_found", "Instance not found");
        assert_eq!(err.error, "not_found");
        assert_eq!(err.message, "Instance not found");
    }

    #[test]
    fn api_error_from_string() {
        let err = ApiError::new(String::from("invalid_id"), String::from("Bad ID format"));
        assert_eq!(err.error, "invalid_id");
        assert_eq!(err.message, "Bad ID format");
    }

    #[test]
    fn api_error_roundtrip() {
        let err = ApiError::new("at_capacity", "Server at capacity");
        let serialized = serde_json::to_string(&err).unwrap();
        let deserialized: ApiError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(err.error, deserialized.error);
        assert_eq!(err.message, deserialized.message);
    }

    #[test]
    fn api_error_deserialize() {
        let json_val = json!({"error": "timeout", "message": "Request timed out"});
        let err: ApiError = serde_json::from_value(json_val).unwrap();
        assert_eq!(err.error, "timeout");
        assert_eq!(err.message, "Request timed out");
    }

    #[test]
    fn api_error_all_standard_codes() {
        let error_codes = [
            ("not_found", "Resource not found"),
            ("invalid_id", "Invalid ID format"),
            ("at_capacity", "Server at capacity"),
            ("timeout", "Request timed out"),
            ("internal_error", "Internal server error"),
            ("invalid_request", "Malformed request body"),
        ];
        for (code, msg) in error_codes {
            let err = ApiError::new(code, msg);
            let serialized = serde_json::to_string(&err).unwrap();
            let deserialized: ApiError = serde_json::from_str(&serialized).unwrap();
            assert_eq!(err.error, deserialized.error);
        }
    }
}

mod parse_error_flow {
    use super::*;

    #[test]
    fn parse_error_empty_workflow_name() {
        let err = ParseError::EmptyWorkflowName;
        assert_eq!(err.to_string(), "workflow_name is empty string");
    }

    #[test]
    fn parse_error_invalid_workflow_name_format() {
        let err = ParseError::InvalidWorkflowNameFormat;
        assert!(err.to_string().contains("pattern"));
    }

    #[test]
    fn parse_error_empty_signal_name() {
        let err = ParseError::EmptySignalName;
        assert_eq!(err.to_string(), "signal_name is empty string");
    }

    #[test]
    fn parse_error_invalid_signal_name_format() {
        let err = ParseError::InvalidSignalNameFormat;
        assert!(err.to_string().contains("signal_name"));
    }

    #[test]
    fn parse_error_invalid_ulid_format() {
        let err = ParseError::InvalidUlidFormat;
        assert!(err.to_string().contains("Crockford base32"));
    }

    #[test]
    fn parse_error_invalid_timestamp_format() {
        let err = ParseError::InvalidTimestampFormat;
        assert!(err.to_string().contains("RFC3339"));
    }

    #[test]
    fn parse_error_internal_error() {
        let err = ParseError::InternalError("regex failed".to_string());
        assert!(err.to_string().contains("regex failed"));
    }

    #[test]
    fn parse_error_unknown_status_variant() {
        let err = ParseError::UnknownStatusVariant;
        assert!(err.to_string().contains("unknown"));
    }
}

mod validation_error_flow {
    use super::*;

    #[test]
    fn validation_error_invalid_retry_after() {
        let err = ValidationError::InvalidRetryAfterSeconds;
        assert!(err.to_string().contains("retry_after_seconds"));
    }

    #[test]
    fn validation_error_invalid_status_transition() {
        let err = ValidationError::InvalidStatusTransition;
        assert!(err.to_string().contains("transition"));
    }

    #[test]
    fn validation_error_invalid_current_step() {
        let err = ValidationError::InvalidCurrentStep;
        assert!(err.to_string().contains("current_step"));
    }
}

mod invariant_violation_flow {
    use super::*;

    #[test]
    fn invariant_violation_updated_before_started() {
        let err = InvariantViolation::UpdatedBeforeStarted;
        assert!(err.to_string().contains("updated_at"));
    }

    #[test]
    fn invariant_violation_entries_not_sorted() {
        let err = InvariantViolation::EntriesNotSorted;
        assert!(err.to_string().contains("ascending"));
    }

    #[test]
    fn invariant_violation_invalid_retry_for_error_type() {
        let err = InvariantViolation::InvalidRetryForErrorType;
        assert!(err.to_string().contains("retry"));
    }

    #[test]
    fn invariant_violation_invocation_id_modified() {
        let err = InvariantViolation::InvocationIdModified;
        assert!(err.to_string().contains("immutable"));
    }

    #[test]
    fn invariant_violation_invalid_status_for_response() {
        let err = InvariantViolation::InvalidStatusForResponse;
        assert!(err.to_string().contains("running"));
    }
}

mod helper_functions_flow {
    use super::*;

    #[test]
    fn is_retryable_error_cases() {
        assert!(is_retryable_error("at_capacity"));
        assert!(is_retryable_error("rate_limited"));
        assert!(!is_retryable_error("not_found"));
        assert!(!is_retryable_error("invalid_id"));
        assert!(!is_retryable_error(""));
        assert!(!is_retryable_error("unknown_error"));
    }

    #[test]
    fn is_sorted_edge_cases() {
        assert!(is_sorted(Vec::<i32>::new().into_iter()));
        assert!(is_sorted(std::iter::once(1)));
        assert!(is_sorted(vec![1, 2, 3, 4, 5].into_iter()));
        assert!(!is_sorted(vec![5, 4, 3, 2, 1].into_iter()));
        assert!(!is_sorted(vec![1, 2, 1, 3].into_iter()));
        assert!(is_sorted(vec![1, 1, 2, 2, 3].into_iter()));
        assert!(is_sorted(vec![5, 5, 5].into_iter()));
    }
}

mod edge_cases {
    use super::*;

    #[test]
    fn large_sequence_numbers() {
        let entry = TimelineEntry {
            sequence: u64::MAX,
            timestamp_ms: u64::MAX,
            event_type: "max_values".to_string(),
            payload: json!({}),
        };
        let serialized = serde_json::to_string(&entry).unwrap();
        let deserialized: TimelineEntry = serde_json::from_str(&serialized).unwrap();
        assert_eq!(entry.sequence, deserialized.sequence);
    }

    #[test]
    fn empty_namespace_and_workflow_type() {
        let req = V3StartRequest {
            namespace: "".to_string(),
            workflow_type: "".to_string(),
            paradigm: "fsm".to_string(),
            input: json!({}),
            instance_id: None,
            dedupe_key: None,
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: V3StartRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req.namespace, deserialized.namespace);
    }

    #[test]
    fn complex_nested_payload() {
        let payload = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "array": [1, "two", true, null],
                        "nested_array": [[1, 2], [3, 4]],
                        "mixed": {"key": [1, 2, 3]}
                    }
                }
            },
            "list": [
                {"id": 1, "name": "first"},
                {"id": 2, "name": "second"}
            ]
        });
        let req = V3SignalRequest {
            signal_name: "complex".to_string(),
            payload,
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: V3SignalRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.payload["level1"]["level2"]["level3"]["array"],
            json!([1, "two", true, null])
        );
    }

    #[test]
    fn special_characters_in_strings() {
        let req = V3StartRequest {
            namespace: "ns-with-dash".to_string(),
            workflow_type: "wf_with_underscore".to_string(),
            paradigm: "fsm".to_string(),
            input: json!({
                "special": "quotes\"and\nnewlines\tand\ttabs"
            }),
            instance_id: None,
            dedupe_key: None,
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: V3StartRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req.namespace, deserialized.namespace);
    }

    #[test]
    fn unicode_in_payload() {
        let payload = json!({
            "emoji": "hello 👋 world 🌍",
            "chinese": "你好世界",
            "japanese": "こんにちは世界",
            "arabic": "مرحبا بالعالم",
            "mixed": "a1α2β3γ"
        });
        let req = V3SignalRequest {
            signal_name: "unicode_test".to_string(),
            payload,
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: V3SignalRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.payload["emoji"], "hello 👋 world 🌍");
    }

    #[test]
    fn timeline_with_zero_values() {
        let resp = TimelineResponse {
            instance_id: "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            entries: vec![TimelineEntry {
                sequence: 0,
                timestamp_ms: 0,
                event_type: "".to_string(),
                payload: json!(null),
            }],
            total_replayed: 0,
            replay_error_count: 0,
            truncated: false,
        };
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: TimelineResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(resp.entries[0].sequence, deserialized.entries[0].sequence);
    }
}

mod error_handling_edge_cases {
    use super::*;
    use vo_api::types::names::InvocationId;

    #[test]
    fn api_error_with_retry_after() {
        let retry = RetryAfterSeconds::new(30).expect("valid retry seconds");
        let result =
            ApiError::new_with_retry_validation("at_capacity", "Server at capacity", Some(retry));
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.error, "at_capacity");
        assert!(resp.retry_after_seconds.is_some());
    }

    #[test]
    fn api_error_retryable_without_retry_field_fails() {
        let result = ApiError::new_with_retry_validation("at_capacity", "Server at capacity", None);
        assert!(result.is_err());
    }

    #[test]
    fn api_error_non_retryable_with_retry_field_fails() {
        let retry = RetryAfterSeconds::new(30).expect("valid retry seconds");
        let result = ApiError::new_with_retry_validation("not_found", "Not found", Some(retry));
        assert!(result.is_err());
    }

    #[test]
    fn start_workflow_response_validation() {
        let resp = StartWorkflowResponse {
            invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .expect("valid ulid"),
            workflow_name: "test_wf".to_string(),
            status: WorkflowStatusValue::Running,
            started_at: Timestamp::new("2024-01-15T10:30:00Z").expect("valid timestamp"),
        };
        assert!(resp.validate().is_ok());
    }

    #[test]
    fn start_workflow_response_validation_fails_for_non_running() {
        let resp = StartWorkflowResponse {
            invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .expect("valid ulid"),
            workflow_name: "test_wf".to_string(),
            status: WorkflowStatusValue::Completed,
            started_at: Timestamp::new("2024-01-15T10:30:00Z").expect("valid timestamp"),
        };
        assert!(resp.validate().is_err());
    }
}
