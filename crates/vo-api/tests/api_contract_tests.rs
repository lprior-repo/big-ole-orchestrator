use serde_json::json;
use vo_api::types::{ApiError, V3SignalRequest, V3StartRequest, V3StartResponse, V3StatusResponse};

mod v3_types {
    use super::*;

    mod start_request {
        use super::*;

        #[test]
        fn deserialize_minimal_request() {
            let json = json!({
                "namespace": "payments",
                "workflow_type": "checkout",
                "paradigm": "fsm",
                "input": {"order_id": "ord_123"}
            });
            let req: V3StartRequest = serde_json::from_value(json).unwrap();
            assert_eq!(req.namespace, "payments");
            assert_eq!(req.workflow_type, "checkout");
            assert_eq!(req.paradigm, "fsm");
            assert_eq!(req.input["order_id"], "ord_123");
            assert!(req.instance_id.is_none());
            assert!(req.dedupe_key.is_none());
        }

        #[test]
        fn deserialize_full_request_with_all_fields() {
            let json = json!({
                "namespace": "orders",
                "workflow_type": "process_order",
                "paradigm": "dag",
                "input": {"items": ["a", "b"]},
                "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "dedupe_key": "dedupe-abc-123"
            });
            let req: V3StartRequest = serde_json::from_value(json).unwrap();
            assert_eq!(req.namespace, "orders");
            assert_eq!(req.workflow_type, "process_order");
            assert_eq!(req.paradigm, "dag");
            assert_eq!(
                req.instance_id,
                Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string())
            );
            assert_eq!(req.dedupe_key, Some("dedupe-abc-123".to_string()));
        }

        #[test]
        fn serialize_roundtrip_preserves_fields() {
            let req = V3StartRequest {
                namespace: "test_ns".to_string(),
                workflow_type: "test_wf".to_string(),
                paradigm: "procedural".to_string(),
                input: json!({"key": "value"}),
                instance_id: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
                dedupe_key: Some("key123".to_string()),
            };
            let serialized = serde_json::to_string(&req).unwrap();
            let deserialized: V3StartRequest = serde_json::from_str(&serialized).unwrap();
            assert_eq!(req.namespace, deserialized.namespace);
            assert_eq!(req.workflow_type, deserialized.workflow_type);
            assert_eq!(req.paradigm, deserialized.paradigm);
            assert_eq!(req.instance_id, deserialized.instance_id);
            assert_eq!(req.dedupe_key, deserialized.dedupe_key);
        }

        #[test]
        fn instance_id_not_serialized_when_none() {
            let req = V3StartRequest {
                namespace: "ns".to_string(),
                workflow_type: "wf".to_string(),
                paradigm: "fsm".to_string(),
                input: json!({}),
                instance_id: None,
                dedupe_key: None,
            };
            let json_str = serde_json::to_string(&req).unwrap();
            assert!(!json_str.contains("instance_id"));
            assert!(!json_str.contains("dedupe_key"));
        }
    }

    mod start_response {
        use super::*;

        #[test]
        fn deserialize_response() {
            let json = json!({
                "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "namespace": "payments",
                "workflow_type": "checkout"
            });
            let resp: V3StartResponse = serde_json::from_value(json).unwrap();
            assert_eq!(resp.instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
            assert_eq!(resp.namespace, "payments");
            assert_eq!(resp.workflow_type, "checkout");
        }

        #[test]
        fn serialize_response() {
            let resp = V3StartResponse {
                instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
                namespace: "orders".to_string(),
                workflow_type: "process".to_string(),
            };
            let json = serde_json::to_value(&resp).unwrap();
            assert_eq!(json["instance_id"], "01ARZ3NDEKTSV4RRFFQ69G5FAV");
            assert_eq!(json["namespace"], "orders");
            assert_eq!(json["workflow_type"], "process");
        }
    }

    mod status_response {
        use super::*;

        #[test]
        fn deserialize_status_response() {
            let json = json!({
                "instance_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "namespace": "payments",
                "workflow_type": "checkout",
                "paradigm": "fsm",
                "phase": "live",
                "events_applied": 42
            });
            let resp: V3StatusResponse = serde_json::from_value(json).unwrap();
            assert_eq!(resp.instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
            assert_eq!(resp.namespace, "payments");
            assert_eq!(resp.paradigm, "fsm");
            assert_eq!(resp.phase, "live");
            assert_eq!(resp.events_applied, 42);
        }

        #[test]
        fn serialize_status_response() {
            let resp = V3StatusResponse {
                instance_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
                namespace: "ns".to_string(),
                workflow_type: "wf".to_string(),
                paradigm: "dag".to_string(),
                phase: "replay".to_string(),
                events_applied: 100,
            };
            let json = serde_json::to_value(&resp).unwrap();
            assert_eq!(json["events_applied"], 100);
            assert_eq!(json["phase"], "replay");
        }
    }

    mod signal_request {
        use super::*;

        #[test]
        fn deserialize_signal_request() {
            let json = json!({
                "signal_name": "payment_approved",
                "payload": {"amount": 100}
            });
            let req: V3SignalRequest = serde_json::from_value(json).unwrap();
            assert_eq!(req.signal_name, "payment_approved");
            assert_eq!(req.payload["amount"], 100);
        }

        #[test]
        fn serialize_signal_request() {
            let req = V3SignalRequest {
                signal_name: "cancel".to_string(),
                payload: json!({"reason": "user_requested"}),
            };
            let serialized = serde_json::to_string(&req).unwrap();
            let deserialized: V3SignalRequest = serde_json::from_str(&serialized).unwrap();
            assert_eq!(req.signal_name, deserialized.signal_name);
        }
    }

    mod api_error {
        use super::*;

        #[test]
        fn new_creates_error_with_code_and_message() {
            let err = ApiError::new("not_found", "Instance not found");
            assert_eq!(err.error, "not_found");
            assert_eq!(err.message, "Instance not found");
        }

        #[test]
        fn new_accepts_string_conversions() {
            let err = ApiError::new(String::from("invalid_id"), String::from("Bad ID format"));
            assert_eq!(err.error, "invalid_id");
            assert_eq!(err.message, "Bad ID format");
        }

        #[test]
        fn serialize_error() {
            let err = ApiError::new("at_capacity", "Server at capacity");
            let json = serde_json::to_value(&err).unwrap();
            assert_eq!(json["error"], "at_capacity");
            assert_eq!(json["message"], "Server at capacity");
        }

        #[test]
        fn deserialize_error() {
            let json = json!({"error": "timeout", "message": "Request timed out"});
            let err: ApiError = serde_json::from_value(json).unwrap();
            assert_eq!(err.error, "timeout");
            assert_eq!(err.message, "Request timed out");
        }
    }
}

mod v1_types {
    use super::json;
    use super::*;
    use vo_api::types::v1::*;

    mod workflow_status_value {
        use super::*;

        #[test]
        fn serialize_lowercase() {
            let values = [
                (WorkflowStatusValue::Pending, "pending"),
                (WorkflowStatusValue::Running, "running"),
                (WorkflowStatusValue::Completed, "completed"),
                (WorkflowStatusValue::Failed, "failed"),
                (WorkflowStatusValue::Cancelled, "cancelled"),
            ];
            for (val, expected) in values {
                let json = serde_json::to_value(&val).unwrap();
                assert_eq!(json, expected);
            }
        }

        #[test]
        fn deserialize_lowercase() {
            let json = json!("running");
            let val: WorkflowStatusValue = serde_json::from_value(json).unwrap();
            assert_eq!(val, WorkflowStatusValue::Running);
        }
    }

    mod journal_entry {
        use super::*;

        #[test]
        fn serialize_run_entry() {
            let entry = JournalEntry {
                seq: 1,
                entry_type: JournalEntryType::Run,
                name: Some("checkout".to_string()),
                input: Some(json!({"order_id": "123"})),
                output: None,
                timestamp: None,
                duration_ms: Some(150),
                fire_at: None,
                status: None,
            };
            let json = serde_json::to_value(&entry).unwrap();
            assert_eq!(json["seq"], 1);
            assert_eq!(json["type"], "Run");
            assert_eq!(json["name"], "checkout");
            assert!(json["input"].is_object());
            assert!(json["output"].is_null());
        }

        #[test]
        fn serialize_wait_entry() {
            let entry = JournalEntry {
                seq: 2,
                entry_type: JournalEntryType::Wait,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: Some("2024-01-15T10:30:00Z".to_string()),
                status: None,
            };
            let json = serde_json::to_value(&entry).unwrap();
            assert_eq!(json["type"], "Wait");
            assert!(json["fire_at"].is_string());
        }

        #[test]
        fn skip_serializing_none_fields() {
            let entry = JournalEntry {
                seq: 0,
                entry_type: JournalEntryType::Run,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            };
            let json_str = serde_json::to_string(&entry).unwrap();
            assert!(!json_str.contains("\"name\""));
            assert!(!json_str.contains("\"input\""));
            assert!(!json_str.contains("\"output\""));
        }
    }
}

mod error_types {
    use vo_api::types::errors::*;

    mod parse_error {
        use super::*;

        #[test]
        fn empty_workflow_name_error_message() {
            let err = ParseError::EmptyWorkflowName;
            assert_eq!(err.to_string(), "workflow_name is empty string");
        }

        #[test]
        fn invalid_workflow_name_format_error_message() {
            let err = ParseError::InvalidWorkflowNameFormat;
            assert!(err.to_string().contains("pattern"));
        }

        #[test]
        fn empty_signal_name_error_message() {
            let err = ParseError::EmptySignalName;
            assert_eq!(err.to_string(), "signal_name is empty string");
        }

        #[test]
        fn invalid_signal_name_format_error_message() {
            let err = ParseError::InvalidSignalNameFormat;
            assert!(err.to_string().contains("signal_name"));
        }

        #[test]
        fn invalid_ulid_format_error_message() {
            let err = ParseError::InvalidUlidFormat;
            assert!(err.to_string().contains("Crockford base32"));
        }

        #[test]
        fn invalid_timestamp_format_error_message() {
            let err = ParseError::InvalidTimestampFormat;
            assert!(err.to_string().contains("RFC3339"));
        }

        #[test]
        fn internal_error_with_message() {
            let err = ParseError::InternalError("regex failed".to_string());
            assert!(err.to_string().contains("regex failed"));
        }
    }

    mod validation_error {
        use super::*;

        #[test]
        fn invalid_retry_after_seconds_error_message() {
            let err = ValidationError::InvalidRetryAfterSeconds;
            assert!(err.to_string().contains("retry_after_seconds"));
        }

        #[test]
        fn invalid_status_transition_error_message() {
            let err = ValidationError::InvalidStatusTransition;
            assert!(err.to_string().contains("transition"));
        }

        #[test]
        fn invalid_current_step_error_message() {
            let err = ValidationError::InvalidCurrentStep;
            assert!(err.to_string().contains("current_step"));
        }
    }

    mod invariant_violation {
        use super::*;

        #[test]
        fn updated_before_started_error_message() {
            let err = InvariantViolation::UpdatedBeforeStarted;
            assert!(err.to_string().contains("updated_at"));
        }

        #[test]
        fn entries_not_sorted_error_message() {
            let err = InvariantViolation::EntriesNotSorted;
            assert!(err.to_string().contains("ascending"));
        }

        #[test]
        fn invalid_retry_for_error_type_error_message() {
            let err = InvariantViolation::InvalidRetryForErrorType;
            assert!(err.to_string().contains("retry"));
        }

        #[test]
        fn invocation_id_modified_error_message() {
            let err = InvariantViolation::InvocationIdModified;
            assert!(err.to_string().contains("immutable"));
        }

        #[test]
        fn invalid_status_for_response_error_message() {
            let err = InvariantViolation::InvalidStatusForResponse;
            assert!(err.to_string().contains("running"));
        }
    }
}

mod helpers {
    use vo_api::types::helpers::*;

    mod is_retryable_error {
        use super::*;

        #[test]
        fn at_capacity_is_retryable() {
            assert!(is_retryable_error("at_capacity"));
        }

        #[test]
        fn not_found_is_not_retryable() {
            assert!(!is_retryable_error("not_found"));
        }

        #[test]
        fn invalid_id_is_not_retryable() {
            assert!(!is_retryable_error("invalid_id"));
        }

        #[test]
        fn empty_string_is_not_retryable() {
            assert!(!is_retryable_error(""));
        }

        #[test]
        fn unknown_errors_are_not_retryable() {
            assert!(!is_retryable_error("something_else"));
        }
    }

    mod is_sorted {
        use super::*;

        #[test]
        fn empty_iterator_is_sorted() {
            let empty: Vec<i32> = vec![];
            assert!(is_sorted(empty.into_iter()));
        }

        #[test]
        fn single_element_is_sorted() {
            assert!(is_sorted(std::iter::once(1)));
        }

        #[test]
        fn ascending_integers_are_sorted() {
            assert!(is_sorted(vec![1, 2, 3, 4, 5].into_iter()));
        }

        #[test]
        fn descending_integers_are_not_sorted() {
            assert!(!is_sorted(vec![5, 4, 3, 2, 1].into_iter()));
        }

        #[test]
        fn unsorted_with_duplicates_are_not_sorted() {
            assert!(!is_sorted(vec![1, 2, 1, 3].into_iter()));
        }

        #[test]
        fn sorted_with_duplicates_are_sorted() {
            assert!(is_sorted(vec![1, 1, 2, 2, 3].into_iter()));
        }

        #[test]
        fn equal_elements_are_sorted() {
            assert!(is_sorted(vec![5, 5, 5].into_iter()));
        }
    }
}
