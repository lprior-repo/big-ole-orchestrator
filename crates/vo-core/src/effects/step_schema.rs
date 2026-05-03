//! Step result payload validation against a step schema.
//!
//! Validates that [`vo_executor::StepResult`] payloads conform to a declared
//! schema before they propagate through the event stream.
//!
//! # Domain
//!
//! Each step declares a schema describing the fields expected in its result
//! payload. Validation walks the schema, checks field presence, verifies
//! types, and optionally flags extra fields.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The expected type of a schema field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    /// A JSON string value.
    String,
    /// A JSON number (integer or float).
    Number,
    /// A JSON boolean.
    Bool,
    /// A JSON object (`{}`).
    Object,
    /// A JSON array (`[]`).
    Array,
}

impl FieldType {
    /// Check whether the given JSON [`Value`] matches this field type.
    #[must_use]
    pub fn matches(&self, value: &Value) -> bool {
        match (self, value) {
            (FieldType::String, Value::String(_)) => true,
            (FieldType::Number, Value::Number(_)) => true,
            (FieldType::Bool, Value::Bool(_)) => true,
            (FieldType::Object, Value::Object(_)) => true,
            (FieldType::Array, Value::Array(_)) => true,
            _ => false,
        }
    }

    /// Return a human-readable name for this type.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Number => "number",
            FieldType::Bool => "bool",
            FieldType::Object => "object",
            FieldType::Array => "array",
        }
    }
}

/// Specification for a single field within a step schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSpec {
    /// The JSON key name for this field.
    pub name: String,
    /// The expected JSON type.
    pub field_type: FieldType,
    /// Whether this field is optional (not required to be present).
    pub optional: bool,
}

impl FieldSpec {
    /// Create a new required field specification.
    #[must_use]
    pub fn required(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            optional: false,
        }
    }

    /// Create a new optional field specification.
    #[must_use]
    pub fn optional(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            optional: true,
        }
    }
}

/// Schema that defines the expected structure of a step result payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSchema {
    /// List of fields and their expected types.
    pub expected_fields: Vec<FieldSpec>,
    /// Set of field names that are required (subset or superset of expected_fields).
    pub required: Vec<String>,
    /// When `true`, fields not in `expected_fields` are flagged as errors.
    pub strict_mode: bool,
}

impl StepSchema {
    /// Create a new schema from a list of field specs.
    ///
    /// All fields listed in `expected_fields` are treated as required
    /// unless explicitly marked `optional = false` on the spec.
    #[must_use]
    pub fn new(expected_fields: Vec<FieldSpec>) -> Self {
        let required: Vec<String> = expected_fields
            .iter()
            .filter(|f| !f.optional)
            .map(|f| f.name.clone())
            .collect();
        Self {
            expected_fields,
            required,
            strict_mode: true,
        }
    }

    /// Create a schema in lenient mode (extra fields are ignored).
    #[must_use]
    pub fn new_lenient(expected_fields: Vec<FieldSpec>) -> Self {
        let mut schema = Self::new(expected_fields);
        schema.strict_mode = false;
        schema
    }

    /// Validate a JSON payload against this schema.
    ///
    /// Returns `Ok(())` if the payload conforms, or a [`ValidateError`] containing
    /// all validation errors (aggregated, not fail-fast).
    ///
    /// # Arguments
    ///
    /// * `payload` - The JSON value to validate (typically from `StepResult.output`).
    /// * `step` - The step identifier, used in error messages.
    pub fn validate(&self, payload: &Value, step: &str) -> Result<(), ValidateError> {
        let mut errors = Vec::new();

        let obj = match payload {
            Value::Object(obj) => obj,
            _ => {
                return Err(ValidateError {
                    step: step.to_string(),
                    errors: vec![ValidationError::TypeMismatch {
                        field: "(root)".to_string(),
                        expected: "object".to_string(),
                        actual: payload_type_name(payload).to_string(),
                    }],
                });
            }
        };

        // Check required fields
        for field_name in &self.required {
            if !obj.contains_key(field_name.as_str()) {
                let spec = self
                    .expected_fields
                    .iter()
                    .find(|f| f.name == *field_name);
                let expected_type = spec
                    .map(|s| s.field_type.name().to_string())
                    .unwrap_or_else(|| "(unknown)".to_string());
                errors.push(ValidationError::MissingRequiredField {
                    field: field_name.clone(),
                    step: step.to_string(),
                });
                let _ = expected_type;
            }
        }

        // Check each expected field
        for spec in &self.expected_fields {
            if let Some(value) = obj.get(&spec.name) {
                if !spec.field_type.matches(value) {
                    errors.push(ValidationError::TypeMismatch {
                        field: spec.name.clone(),
                        expected: spec.field_type.name().to_string(),
                        actual: value_type_name(value),
                    });
                }
            } else if !spec.optional {
                // Already caught by required check, but also add here for completeness
                if !self.required.contains(&spec.name) {
                    errors.push(ValidationError::MissingRequiredField {
                        field: spec.name.clone(),
                        step: step.to_string(),
                    });
                }
            }
        }

        // Check for extra fields in strict mode
        if self.strict_mode {
            let expected_names: std::collections::HashSet<&str> =
                self.expected_fields.iter().map(|f| f.name.as_str()).collect();
            for key in obj.keys() {
                if !expected_names.contains(key.as_str()) {
                    errors.push(ValidationError::ExtraFields {
                        fields: vec![key.clone()],
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidateError {
                step: step.to_string(),
                errors,
            })
        }
    }

    /// Validate a string payload (parses as JSON first).
    ///
    /// Returns a [`ParseError`] if the string is not valid JSON, or a
    /// [`ValidateError`] if validation fails.
    pub fn validate_str(&self, payload: &str, step: &str) -> Result<(), SchemaError> {
        let value: Value = serde_json::from_str(payload).map_err(|e| SchemaError::Parse {
            error: e.to_string(),
            step: step.to_string(),
        })?;
        self.validate(&value, step)
            .map_err(|e| SchemaError::Validation(e))
    }
}

/// Errors that can occur during schema validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// The payload string is not valid JSON.
    #[error("failed to parse payload as JSON: {error} (step: {step})")]
    Parse { error: String, step: String },

    /// The payload failed schema validation.
    #[error("validation failed: {0}")]
    Validation(#[from] ValidateError),
}

/// Aggregated validation error for a step result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("step '{step}' failed validation: {errors:?}")]
pub struct ValidateError {
    /// The step that failed validation.
    pub step: String,
    /// All individual validation errors found.
    pub errors: Vec<ValidationError>,
}

/// A single validation failure within a [`ValidateError`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// A required field is missing from the payload.
    #[error("missing required field '{field}'")]
    MissingRequiredField { field: String, step: String },

    /// A field's value does not match the expected type.
    #[error("type mismatch for field '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    /// Fields present in the payload but not declared in the schema (strict mode only).
    #[error("extra fields not in schema: {fields:?}")]
    ExtraFields { fields: Vec<String> },
}

/// Return the JSON type name for a [`Value`].
fn payload_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn value_type_name(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "bool".to_string(),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer".to_string()
            } else {
                "number".to_string()
            }
        }
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_schema() -> StepSchema {
        StepSchema::new(vec![
            FieldSpec::required("name", FieldType::String),
            FieldSpec::required("count", FieldType::Number),
            FieldSpec::required("active", FieldType::Bool),
            FieldSpec::optional("metadata", FieldType::Object),
        ])
    }

    fn make_schema_strict() -> StepSchema {
        let mut s = make_schema();
        s.strict_mode = true;
        s
    }

    fn make_schema_lenient() -> StepSchema {
        let mut s = make_schema();
        s.strict_mode = false;
        s
    }

    fn valid_payload() -> Value {
        json!({
            "name": "test",
            "count": 42,
            "active": true
        })
    }

    // --- Valid payload ---

    #[test]
    fn test_validate_valid_payload() {
        let schema = make_schema();
        assert!(schema.validate(&valid_payload(), "step-1").is_ok());
    }

    #[test]
    fn test_validate_valid_payload_with_optional() {
        let schema = make_schema();
        let payload = json!({
            "name": "test",
            "count": 42,
            "active": true,
            "metadata": {"key": "val"}
        });
        assert!(schema.validate(&payload, "step-1").is_ok());
    }

    // --- Missing required field ---

    #[test]
    fn test_validate_missing_required() {
        let schema = make_schema();
        let payload = json!({
            "count": 42,
            "active": true
        });
        let err = schema.validate(&payload, "step-2").unwrap_err();
        assert_eq!(err.step, "step-2");
        assert!(matches!(
            &err.errors[..],
            [ValidationError::MissingRequiredField { field, .. }] if field == "name"
        ));
    }

    // --- Type mismatch ---

    #[test]
    fn test_validate_type_mismatch() {
        let schema = make_schema();
        let payload = json!({
            "name": 123,
            "count": 42,
            "active": true
        });
        let err = schema.validate(&payload, "step-3").unwrap_err();
        assert!(matches!(
            &err.errors[..],
            [ValidationError::TypeMismatch { field, expected, .. }]
                if field == "name" && expected == "string"
        ));
    }

    #[test]
    fn test_validate_multiple_type_mismatches() {
        let schema = make_schema();
        let payload = json!({
            "name": 123,
            "count": "not_a_number",
            "active": true
        });
        let err = schema.validate(&payload, "step-3b").unwrap_err();
        assert_eq!(err.errors.len(), 2);
    }

    // --- Extra fields in strict mode ---

    #[test]
    fn test_validate_extra_fields_strict_mode() {
        let schema = make_schema_strict();
        let payload = json!({
            "name": "test",
            "count": 42,
            "active": true,
            "unknown_field": "extra"
        });
        let err = schema.validate(&payload, "step-4").unwrap_err();
        assert!(matches!(
            &err.errors[..],
            [ValidationError::ExtraFields { fields }] if fields == ["unknown_field"]
        ));
    }

    #[test]
    fn test_validate_extra_fields_strict_multiple() {
        let schema = make_schema_strict();
        let payload = json!({
            "name": "test",
            "count": 42,
            "active": true,
            "extra1": "a",
            "extra2": "b"
        });
        let err = schema.validate(&payload, "step-4b").unwrap_err();
        assert!(matches!(&err.errors[..],
            [ValidationError::ExtraFields { fields }, ValidationError::ExtraFields { fields: fields2, .. }]
            if fields.len() == 1 && fields2.len() == 1
        ));
    }

    // --- Extra fields in lenient mode ---

    #[test]
    fn test_validate_extra_fields_lenient_mode() {
        let schema = make_schema_lenient();
        let payload = json!({
            "name": "test",
            "count": 42,
            "active": true,
            "extra_field": "should_be_ignored"
        });
        assert!(schema.validate(&payload, "step-5").is_ok());
    }

    // --- Nested object validation ---

    #[test]
    fn test_validate_nested_object() {
        let schema = StepSchema::new(vec![
            FieldSpec::required("data", FieldType::Object),
        ]);
        let payload = json!({
            "data": {"nested": "value", "number": 42}
        });
        assert!(schema.validate(&payload, "step-6").is_ok());
    }

    #[test]
    fn test_validate_nested_object_type_mismatch() {
        let schema = StepSchema::new(vec![
            FieldSpec::required("data", FieldType::Object),
        ]);
        let payload = json!({
            "data": "not_an_object"
        });
        let err = schema.validate(&payload, "step-6b").unwrap_err();
        assert!(matches!(
            &err.errors[..],
            [ValidationError::TypeMismatch { field, expected, actual }]
                if field == "data" && expected == "object" && actual == "string"
        ));
    }

    // --- Root type validation ---

    #[test]
    fn test_validate_root_not_object() {
        let schema = make_schema();
        let payload = json!("just_a_string");
        let err = schema.validate(&payload, "step-7").unwrap_err();
        assert!(matches!(
            &err.errors[..],
            [ValidationError::TypeMismatch { field, .. }] if field == "(root)"
        ));
    }

    #[test]
    fn test_validate_root_array() {
        let schema = make_schema();
        let payload = json!([1, 2, 3]);
        let err = schema.validate(&payload, "step-7b").unwrap_err();
        assert!(matches!(
            &err.errors[..],
            [ValidationError::TypeMismatch { field, expected, actual }]
                if field == "(root)" && expected == "object" && actual == "array"
        ));
    }

    // --- validate_str ---

    #[test]
    fn test_validate_str_valid() {
        let schema = make_schema();
        assert!(schema.validate_str(
            r#"{"name": "test", "count": 42, "active": true}"#,
            "step-8"
        ).is_ok());
    }

    #[test]
    fn test_validate_str_parse_error() {
        let schema = make_schema();
        let err = schema.validate_str("not json at all", "step-9").unwrap_err();
        assert!(matches!(err, SchemaError::Parse { .. }));
    }

    #[test]
    fn test_validate_str_validation_error() {
        let schema = make_schema();
        let err = schema.validate_str(r#"{"count": 42}"#, "step-10").unwrap_err();
        assert!(matches!(err, SchemaError::Validation(_)));
    }

    // --- Schema construction ---

    #[test]
    fn test_schema_auto_required() {
        let schema = StepSchema::new(vec![
            FieldSpec::required("a", FieldType::String),
            FieldSpec::optional("b", FieldType::Number),
        ]);
        assert_eq!(schema.required, vec!["a"]);
        assert!(schema.strict_mode);
    }

    #[test]
    fn test_schema_new_lenient() {
        let schema = StepSchema::new_lenient(vec![
            FieldSpec::required("x", FieldType::Bool),
        ]);
        assert!(!schema.strict_mode);
    }

    // --- FieldType ---

    #[test]
    fn test_field_type_matches() {
        assert!(FieldType::String.matches(&json!("hello")));
        assert!(FieldType::Number.matches(&json!(42)));
        assert!(FieldType::Bool.matches(&json!(true)));
        assert!(FieldType::Object.matches(&json!({})));
        assert!(FieldType::Array.matches(&json!([])));
    }

    #[test]
    fn test_field_type_no_match() {
        assert!(!FieldType::String.matches(&json!(42)));
        assert!(!FieldType::Number.matches(&json!("hello")));
        assert!(!FieldType::Bool.matches(&json!(0)));
    }

    #[test]
    fn test_field_type_names() {
        assert_eq!(FieldType::String.name(), "string");
        assert_eq!(FieldType::Number.name(), "number");
        assert_eq!(FieldType::Bool.name(), "bool");
        assert_eq!(FieldType::Object.name(), "object");
        assert_eq!(FieldType::Array.name(), "array");
    }

    // --- FieldSpec ---

    #[test]
    fn test_field_spec_required() {
        let spec = FieldSpec::required("foo", FieldType::String);
        assert_eq!(spec.name, "foo");
        assert_eq!(spec.field_type, FieldType::String);
        assert!(!spec.optional);
    }

    #[test]
    fn test_field_spec_optional() {
        let spec = FieldSpec::optional("bar", FieldType::Number);
        assert_eq!(spec.name, "bar");
        assert!(spec.optional);
    }

    #[test]
    fn test_validate_extra_fields_lenient_with_missing_required_still_errors() {
        let schema = make_schema_lenient();
        let payload = json!({
            "name": "test",
            "count": 42,
            "extra": "ignored"
        });
        // missing "active" should still error even in lenient mode
        let err = schema.validate(&payload, "step-lenient-missing").unwrap_err();
        assert!(err.errors.iter().any(|e| matches!(e, ValidationError::MissingRequiredField { field, .. } if field == "active")));
    }

    // --- ValidateError ---

    #[test]
    fn test_validate_error_display() {
        let err = ValidateError {
            step: "s1".to_string(),
            errors: vec![
                ValidationError::MissingRequiredField {
                    field: "x".to_string(),
                    step: "s1".to_string(),
                },
            ],
        };
        let display = format!("{err}");
        assert!(display.contains("s1"));
        assert!(display.contains("x"));
    }

    // --- SchemaError ---

    #[test]
    fn test_schema_error_display_parse() {
        let err = SchemaError::Parse {
            error: "expected value".to_string(),
            step: "s2".to_string(),
        };
        let display = format!("{err}");
        assert!(display.contains("expected value"));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    prop_compose! {
        fn any_valid_field_name()(_ in 1..10usize) -> String {
            let s: String = (0.._)
                .map(|_| b'a' + (rand::random::<u8>() % 26) as u8)
                .map(char::from)
                .collect();
            s
        }
    }

    prop_compose! {
        fn any_valid_string_value()() -> String {
            let s: String = (0..(rand::random::<usize>() % 20))
                .map(|_| b'a' + (rand::random::<u8>() % 26) as u8)
                .map(char::from)
                .collect();
            s
        }
    }

    prop_compose! {
        fn any_valid_number_value()() -> i64 {
            rand::random::<i64>()
        }
    }

    prop_compose! {
        fn any_valid_bool_value()() -> bool {
            rand::random::<bool>()
        }
    }

    proptest! {
        #[test]
        fn proptest_valid_payload_passes(
            name in any_valid_field_name(),
            val in any_valid_string_value(),
        ) {
            let schema = StepSchema::new(vec![
                FieldSpec::required(name.clone(), FieldType::String),
            ]);
            let payload = json!({ name => val });
            let result = schema.validate(&payload, "test-step");
            prop_assert!(result.is_ok());
        }

        #[test]
        fn proptest_missing_required_field_fails(
            name in any_valid_field_name(),
        ) {
            let schema = StepSchema::new(vec![
                FieldSpec::required(name.clone(), FieldType::String),
            ]);
            let payload = json!({ "other_field" => "value" });
            let result = schema.validate(&payload, "test-step");
            prop_assert!(result.is_err());
            let err = result.unwrap_err();
            prop_assert!(err.errors.iter().any(|e| matches!(e, ValidationError::MissingRequiredField { field, .. } if field == &name)));
        }

        #[test]
        fn proptest_type_mismatch_detected(
            name in any_valid_field_name(),
        ) {
            let schema = StepSchema::new(vec![
                FieldSpec::required(name.clone(), FieldType::String),
            ]);
            let payload = json!({ name => 42 });
            let result = schema.validate(&payload, "test-step");
            prop_assert!(result.is_err());
            let err = result.unwrap_err();
            prop_assert!(err.errors.iter().any(|e| matches!(e, ValidationError::TypeMismatch { field, .. } if field == &name)));
        }

        #[test]
        fn proptest_strict_mode_flags_extra_fields(
            req_name in any_valid_field_name(),
            extra_name in any_valid_field_name(),
        ) {
            let schema = StepSchema::new(vec![
                FieldSpec::required(req_name.clone(), FieldType::String),
            ]);
            let mut schema_strict = schema;
            schema_strict.strict_mode = true;
            let payload = json!({
                req_name => "value",
                extra_name => "extra"
            });
            let result = schema_strict.validate(&payload, "test-step");
            prop_assert!(result.is_err());
            let err = result.unwrap_err();
            prop_assert!(err.errors.iter().any(|e| matches!(e, ValidationError::ExtraFields { fields } if fields.contains(&extra_name))));
        }

        #[test]
        fn proptest_lenient_mode_ignores_extra_fields(
            name in any_valid_field_name(),
            val in any_valid_string_value(),
            extra_name in any_valid_field_name(),
        ) {
            let mut schema = StepSchema::new(vec![
                FieldSpec::required(name.clone(), FieldType::String),
            ]);
            schema.strict_mode = false;
            let payload = json!({
                name => val,
                extra_name => "should be ignored"
            });
            let result = schema.validate(&payload, "test-step");
            prop_assert!(result.is_ok());
        }

        #[test]
        fn proptest_nested_object_validated(
            parent_name in any_valid_field_name(),
        ) {
            let schema = StepSchema::new(vec![
                FieldSpec::required(parent_name.clone(), FieldType::Object),
            ]);
            let payload = json!({
                parent_name => { "nested_key" => "nested_val" }
            });
            let result = schema.validate(&payload, "test-step");
            prop_assert!(result.is_ok());
        }

        #[test]
        fn proptest_nested_object_type_check(
            parent_name in any_valid_field_name(),
        ) {
            let schema = StepSchema::new(vec![
                FieldSpec::required(parent_name.clone(), FieldType::Object),
            ]);
            let payload = json!({
                parent_name => "not an object"
            });
            let result = schema.validate(&payload, "test-step");
            prop_assert!(result.is_err());
        }
    }
}
