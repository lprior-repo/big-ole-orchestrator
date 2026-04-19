//! Integration tests for SchemaValidator (ve-ypy84).
//!
//! Tests config schema validation: valid config, invalid type, unknown field.

use std::sync::Arc;

use vo_core::config_hot_reload::{ConfigValidator, Error, FieldDef, FieldType, HotReloadConfig, SchemaValidator};

#[test]
fn schema_validator_accepts_valid_config() {
    let schema = SchemaValidator::new(
        vec![
            FieldDef {
                name: "port".into(),
                field_type: FieldType::Integer,
            },
            FieldDef {
                name: "host".into(),
                field_type: FieldType::String,
            },
        ],
        false,
    );

    let config = serde_json::json!({"port": 8080, "host": "localhost"});
    assert!(schema.validate(&config).is_ok());
}

#[test]
fn schema_validator_rejects_string_when_integer_expected() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({"port": "8080"});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("wrong type"), "error should mention wrong type: {err}");
    assert!(err.contains("port"), "error should name the field: {err}");
}

#[test]
fn schema_validator_rejects_unknown_field() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({"port": 8080, "unknown_key": "oops"});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("unknown field"), "error should mention unknown field: {err}");
    assert!(err.contains("unknown_key"), "error should name the field: {err}");
}

#[test]
fn schema_validator_rejects_integer_when_string_expected() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "host".into(),
            field_type: FieldType::String,
        }],
        false,
    );

    let config = serde_json::json!({"host": 42});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("wrong type"), "error should mention wrong type: {err}");
}

#[test]
fn schema_validator_rejects_missing_required_field() {
    let schema = SchemaValidator::new(
        vec![
            FieldDef {
                name: "port".into(),
                field_type: FieldType::Integer,
            },
            FieldDef {
                name: "host".into(),
                field_type: FieldType::String,
            },
        ],
        false,
    );

    let config = serde_json::json!({"port": 8080});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("missing required field"), "error should mention missing field: {err}");
    assert!(err.contains("host"), "error should name the missing field: {err}");
}

#[test]
fn schema_validator_rejects_non_object_config() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!("not an object");
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("must be a JSON object"), "error should mention object requirement: {err}");
}

#[test]
fn schema_validator_rejects_boolean_when_integer_expected() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({"port": true});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("wrong type"), "error should mention wrong type: {err}");
}

#[test]
fn schema_validator_rejects_float_when_integer_expected() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "count".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({"count": 3.14});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("wrong type"), "error should mention wrong type: {err}");
}

#[test]
fn schema_validator_rejects_null_when_field_expected() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({"port": null});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("wrong type"), "error should mention wrong type: {err}");
}

#[test]
fn schema_validator_rejects_empty_object_when_fields_required() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "version".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("missing required field"), "error should mention missing field: {err}");
}

#[test]
fn schema_validator_accepts_number_field_with_float_value() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "ratio".into(),
            field_type: FieldType::Number,
        }],
        false,
    );

    let config = serde_json::json!({"ratio": 3.14});
    assert!(schema.validate(&config).is_ok());
}

#[test]
fn schema_validator_accepts_boolean_field() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "enabled".into(),
            field_type: FieldType::Boolean,
        }],
        false,
    );

    let config = serde_json::json!({"enabled": true});
    assert!(schema.validate(&config).is_ok());
}

#[test]
fn schema_validator_accepts_object_field() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "database".into(),
            field_type: FieldType::Object,
        }],
        false,
    );

    let config = serde_json::json!({"database": {"url": "postgres://localhost"}});
    assert!(schema.validate(&config).is_ok());
}

#[test]
fn schema_validator_accepts_array_field() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "tags".into(),
            field_type: FieldType::Array,
        }],
        false,
    );

    let config = serde_json::json!({"tags": ["a", "b"]});
    assert!(schema.validate(&config).is_ok());
}

#[test]
fn schema_validator_accepts_integer_field_with_negative_value() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "count".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({"count": -42});
    assert!(schema.validate(&config).is_ok());
}

#[test]
fn schema_validator_allow_unknown_accepts_extra_fields() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        true,
    );

    let config = serde_json::json!({"port": 8080, "extra": "allowed", "more": true});
    assert!(schema.validate(&config).is_ok());
}

#[test]
fn schema_validator_rejects_multiple_unknown_fields() {
    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = serde_json::json!({"port": 8080, "foo": 1, "bar": 2});
    let result = schema.validate(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("unknown field"), "error should mention unknown field: {err}");
}

// ============================================================
// Integration: SchemaValidator + HotReloadConfig via try_update
// ============================================================

#[test]
fn schema_validator_integrates_with_hot_reload_try_update_accepts_valid() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    std::fs::write(&path, "{}").unwrap();

    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = HotReloadConfig::new(
        serde_json::json!({"port": 8080}),
        path,
        Arc::new(schema),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"port": 9090}));
    assert!(result.is_ok());
}

#[test]
fn schema_validator_integrates_with_hot_reload_try_update_rejects_bad_type() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    std::fs::write(&path, "{}").unwrap();

    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = HotReloadConfig::new(
        serde_json::json!({"port": 8080}),
        path,
        Arc::new(schema),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"port": "not_a_number"}));
    assert!(matches!(result, Err(Error::ValidationFailed(_))));
}

#[test]
fn schema_validator_integrates_with_hot_reload_try_update_rejects_unknown_field() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    std::fs::write(&path, "{}").unwrap();

    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = HotReloadConfig::new(
        serde_json::json!({"port": 8080}),
        path,
        Arc::new(schema),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"port": 9090, "extra": true}));
    assert!(matches!(result, Err(Error::ValidationFailed(msg)) if msg.contains("unknown field")));
}

// ============================================================
// Integration: SchemaValidator + HotReloadConfig via reload_from_file
// ============================================================

#[test]
fn schema_validator_reload_from_file_accepts_valid() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    std::fs::write(&path, r#"{"port": 8080}"#).unwrap();

    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = HotReloadConfig::new(
        serde_json::json!({"port": 8080}),
        path.clone(),
        Arc::new(schema),
    )
    .unwrap();

    std::fs::write(&path, r#"{"port": 9090}"#).unwrap();
    let old = config.reload_from_file().unwrap();
    assert_eq!(old, serde_json::json!({"port": 8080}));
    assert_eq!(config.current(), serde_json::json!({"port": 9090}));
}

#[test]
fn schema_validator_reload_from_file_rejects_bad_type() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    std::fs::write(&path, r#"{"port": 8080}"#).unwrap();

    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = HotReloadConfig::new(
        serde_json::json!({"port": 8080}),
        path.clone(),
        Arc::new(schema),
    )
    .unwrap();

    std::fs::write(&path, r#"{"port": "not_a_number"}"#).unwrap();
    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ValidationFailed(msg)) if msg.contains("wrong type")));
    assert_eq!(config.current(), serde_json::json!({"port": 8080}));
}

#[test]
fn schema_validator_reload_from_file_rejects_unknown_field() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    std::fs::write(&path, r#"{"port": 8080}"#).unwrap();

    let schema = SchemaValidator::new(
        vec![FieldDef {
            name: "port".into(),
            field_type: FieldType::Integer,
        }],
        false,
    );

    let config = HotReloadConfig::new(
        serde_json::json!({"port": 8080}),
        path.clone(),
        Arc::new(schema),
    )
    .unwrap();

    std::fs::write(&path, r#"{"port": 9090, "extra": true}"#).unwrap();
    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ValidationFailed(msg)) if msg.contains("unknown field")));
    assert_eq!(config.current(), serde_json::json!({"port": 8080}));
}

#[test]
fn schema_validator_reload_from_file_rejects_missing_required_field() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    std::fs::write(&path, r#"{"port": 8080, "host": "localhost"}"#).unwrap();

    let schema = SchemaValidator::new(
        vec![
            FieldDef {
                name: "port".into(),
                field_type: FieldType::Integer,
            },
            FieldDef {
                name: "host".into(),
                field_type: FieldType::String,
            },
        ],
        false,
    );

    let config = HotReloadConfig::new(
        serde_json::json!({"port": 8080, "host": "localhost"}),
        path.clone(),
        Arc::new(schema),
    )
    .unwrap();

    std::fs::write(&path, r#"{"port": 9090}"#).unwrap();
    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ValidationFailed(msg)) if msg.contains("missing required field")));
    assert_eq!(config.current(), serde_json::json!({"port": 8080, "host": "localhost"}));
}
