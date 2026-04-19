//! Tests for workflow sink validation.

use super::workflow::*;
use vo_types::EffectKind;


    #[test]
    fn known_sinks_default_contains_blob_http_sql() {
        let sinks = KnownSinks::default_sinks();
        assert!(sinks.contains("blob"));
        assert!(sinks.contains("http"));
        assert!(sinks.contains("sql"));
        assert_eq!(sinks.len(), 3);
    }

    #[test]
    fn known_sinks_does_not_contain_unknown() {
        let sinks = KnownSinks::default_sinks();
        assert!(!sinks.contains("unknown-sink"));
        assert!(!sinks.contains(""));
    }

    #[test]
    fn known_sinks_with_custom_sinks() {
        let sinks = KnownSinks::new(["custom1", "custom2"]);
        assert!(sinks.contains("custom1"));
        assert!(sinks.contains("custom2"));
        assert!(!sinks.contains("blob"));
    }

    #[test]
    fn validator_accepts_known_sink() {
        let validator = WorkflowSinkValidator::new();
        assert!(validator.validate_sink("blob").is_ok());
        assert!(validator.validate_sink("http").is_ok());
        assert!(validator.validate_sink("sql").is_ok());
    }

    #[test]
    fn validator_rejects_unknown_sink() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sink("unknown-sink");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UnsupportedSinkError::UnknownSink { .. }));
        assert_eq!(err.error_code(), "unsupported_sink");
    }

    #[test]
    fn validator_rejects_empty_sink() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sink("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UnsupportedSinkError::EmptySink));
        assert_eq!(err.error_code(), "empty_sink");
    }

    #[test]
    fn validator_error_message_contains_sink_and_known() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sink("unknown-sink");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown-sink"));
        assert!(msg.contains("blob"));
        assert!(msg.contains("http"));
        assert!(msg.contains("sql"));
    }

    #[test]
    fn validate_workflow_sinks_convenience_function() {
        assert!(validate_workflow_sinks(["blob", "sql"]).is_ok());
        assert!(validate_workflow_sinks(["unknown"]).is_err());
        assert!(validate_workflow_sinks(["blob", ""]).is_err());
    }

    #[test]
    fn unsupported_sink_error_sink_identifier() {
        let err = UnsupportedSinkError::UnknownSink {
            sink: "test-sink".to_string(),
            known_sinks: "blob, http, sql".to_string(),
        };
        assert_eq!(err.sink_identifier(), Some("test-sink"));

        let empty_err = UnsupportedSinkError::EmptySink;
        assert_eq!(empty_err.sink_identifier(), None);
    }

    #[test]
    fn known_sinks_display() {
        let sinks = KnownSinks::default_sinks();
        let display = format!("{}", sinks);
        assert!(display.contains("blob"));
        assert!(display.contains("http"));
        assert!(display.contains("sql"));
    }

    #[test]
    fn workflow_sink_validator_with_custom_sinks() {
        let custom_sinks = KnownSinks::new(["custom-sink"]);
        let validator = WorkflowSinkValidator::with_sinks(custom_sinks);
        assert!(validator.validate_sink("custom-sink").is_ok());
        assert!(validator.validate_sink("blob").is_err());
    }

    #[test]
    fn validate_multiple_sinks_returns_first_error() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sinks(["blob", "unknown", "sql"]);
        assert!(result.is_err());
    }

    #[test]
    fn validate_multiple_known_sinks_succeeds() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sinks(["blob", "http", "sql"]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_with_all_known_effects_succeeds() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_http_call_maps_to_http_sink() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([EffectKind::HttpCall]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_sql_query_maps_to_sql_sink() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([EffectKind::SqlQuery]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_blob_write_maps_to_blob_sink() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([EffectKind::BlobWrite]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_workflow_effects_with_all_known_effects_succeeds() {
        use vo_types::EffectKind;
        let effect_kinds = [
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ];
        let result = validate_workflow_effects(effect_kinds);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_workflow_effects_rejects_empty_sink() {
        let result = validate_workflow_sinks([""]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_code(), "empty_sink");
    }
