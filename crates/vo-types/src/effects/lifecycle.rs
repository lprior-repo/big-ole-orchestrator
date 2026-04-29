//! Effect lifecycle types: EffectKind, EffectRecord, Receipt, and compression.
//!
//! EffectRecord is the persisted representation of a managed effect.
//! Receipt is the durable execution receipt for committed effects (ADR-041).

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EffectKind {
    HttpCall,
    SqlQuery,
    BlobWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum JsonType {
    String,
    Number,
    Bool,
    Object,
    Array,
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StepSchema {
    pub expected_intent: crate::effects::EffectIntent,
    pub expected_kind: EffectKind,
    pub required_params: Vec<String>,
    pub param_types: HashMap<String, JsonType>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectValidationError {
    #[error("Effect intent {0:?} does not match schema expected {1:?}")]
    IntentMismatch(crate::effects::EffectIntent, crate::effects::EffectIntent),
    #[error("Effect kind {0:?} does not match schema expected {1:?}")]
    KindMismatch(EffectKind, EffectKind),
    #[error("Required param '{name}' is missing from params_json")]
    MissingParam { name: String },
    #[error("Param '{param}' has type {actual:?} but schema expects {expected:?}")]
    TypeMismatch {
        param: String,
        expected: JsonType,
        actual: JsonType,
    },
}

pub fn validate_effect_against_schema(
    effect: &EffectRecord,
    schema: &StepSchema,
) -> Result<(), EffectValidationError> {
    if effect.status != schema.expected_intent {
        return Err(EffectValidationError::IntentMismatch(
            effect.status,
            schema.expected_intent,
        ));
    }
    if effect.kind != schema.expected_kind {
        return Err(EffectValidationError::KindMismatch(
            effect.kind,
            schema.expected_kind,
        ));
    }
    if let serde_json::Value::Object(params) = &effect.params_json {
        for param_name in &schema.required_params {
            if !params.contains_key(param_name) {
                return Err(EffectValidationError::MissingParam {
                    name: param_name.clone(),
                });
            }
        }
        for (param_name, expected_type) in &schema.param_types {
            if let Some(value) = params.get(param_name) {
                let actual_type = match value {
                    serde_json::Value::String(_) => JsonType::String,
                    serde_json::Value::Number(_) => JsonType::Number,
                    serde_json::Value::Bool(_) => JsonType::Bool,
                    serde_json::Value::Object(_) => JsonType::Object,
                    serde_json::Value::Array(_) => JsonType::Array,
                    serde_json::Value::Null => JsonType::Null,
                };
                if actual_type != *expected_type {
                    return Err(EffectValidationError::TypeMismatch {
                        param: param_name.clone(),
                        expected: *expected_type,
                        actual: actual_type,
                    });
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectCompressionError {
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EffectRecord {
    intent_id: String,
    kind: EffectKind,
    params_json: serde_json::Value,
    status: crate::effects::EffectIntent,
    committed_at: Option<crate::types::TimestampMs>,
}

impl EffectKind {
    #[must_use]
    pub const fn all_variants() -> &'static [EffectKind] {
        &[
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ]
    }
}

impl EffectRecord {
    #[must_use]
    pub fn new(
        intent_id: String,
        kind: EffectKind,
        params_json: serde_json::Value,
        status: crate::effects::EffectIntent,
        committed_at: Option<crate::types::TimestampMs>,
    ) -> Option<Self> {
        if intent_id.is_empty() {
            return None;
        }
        Some(Self {
            intent_id,
            kind,
            params_json,
            status,
            committed_at,
        })
    }

    #[must_use]
    pub fn new_with_schema(
        intent_id: String,
        kind: EffectKind,
        params_json: serde_json::Value,
        status: crate::effects::EffectIntent,
        committed_at: Option<crate::types::TimestampMs>,
        schema: &StepSchema,
    ) -> Result<Self, EffectValidationError> {
        let record = Self {
            intent_id: intent_id.clone(),
            kind,
            params_json: params_json.clone(),
            status,
            committed_at,
        };
        if intent_id.is_empty() {
            return Err(EffectValidationError::IntentMismatch(
                status,
                schema.expected_intent,
            ));
        }
        validate_effect_against_schema(&record, schema)?;
        Ok(record)
    }

    #[must_use]
    pub fn intent_id(&self) -> &str {
        &self.intent_id
    }

    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    #[must_use]
    pub fn params_json(&self) -> &serde_json::Value {
        &self.params_json
    }

    #[must_use]
    pub fn status(&self) -> crate::effects::EffectIntent {
        self.status
    }

    #[must_use]
    pub fn committed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.committed_at.as_ref()
    }

    pub fn compress(&self) -> Result<Vec<u8>, EffectCompressionError> {
        let json = serde_json::to_string(self)
            .map_err(|e| EffectCompressionError::SerializationFailed(e.to_string()))?;
        let bytes = json.as_bytes();
        zstd::encode_all(bytes, 0)
            .map_err(|e| EffectCompressionError::CompressionFailed(e.to_string()))
    }

    pub fn decompress(compressed: &[u8]) -> Result<Self, EffectCompressionError> {
        let decompressed = zstd::decode_all(compressed)
            .map_err(|e| EffectCompressionError::DecompressionFailed(e.to_string()))?;
        let json = String::from_utf8(decompressed)
            .map_err(|e| EffectCompressionError::DecompressionFailed(e.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| EffectCompressionError::DeserializationFailed(e.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Receipt {
    effect_id: String,
    connector_type: String,
    connector_version: String,
    external_receipt: serde_json::Value,
    committed_at: crate::types::TimestampMs,
}

impl Receipt {
    #[must_use]
    pub fn new(
        effect_id: String,
        connector_type: String,
        connector_version: String,
        external_receipt: serde_json::Value,
        committed_at: crate::types::TimestampMs,
    ) -> Option<Self> {
        if effect_id.is_empty() || connector_type.is_empty() {
            return None;
        }
        Some(Self {
            effect_id,
            connector_type,
            connector_version,
            external_receipt,
            committed_at,
        })
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }
    #[must_use]
    pub fn connector_type(&self) -> &str {
        &self.connector_type
    }
    #[must_use]
    pub fn connector_version(&self) -> &str {
        &self.connector_version
    }
    #[must_use]
    pub fn external_receipt(&self) -> &serde_json::Value {
        &self.external_receipt
    }
    #[must_use]
    pub fn committed_at(&self) -> crate::types::TimestampMs {
        self.committed_at
    }
}

impl std::fmt::Display for Receipt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Receipt(effect={}, connector={}:{}, at={})",
            self.effect_id, self.connector_type, self.connector_version, self.committed_at
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    #[test]
    fn effectkind_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", EffectKind::HttpCall), "HttpCall");
        assert_eq!(format!("{:?}", EffectKind::SqlQuery), "SqlQuery");
        assert_eq!(format!("{:?}", EffectKind::BlobWrite), "BlobWrite");
    }

    #[rstest]
    #[case(EffectKind::HttpCall, "HttpCall")]
    #[case(EffectKind::SqlQuery, "SqlQuery")]
    #[case(EffectKind::BlobWrite, "BlobWrite")]
    fn effectkind_serializes_and_deserializes_for_all_variants(
        #[case] variant: EffectKind,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: EffectKind = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    #[test]
    fn effectkind_all_variants_returns_three_variants_in_declaration_order() {
        let variants = EffectKind::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], EffectKind::HttpCall);
        assert_eq!(variants[1], EffectKind::SqlQuery);
        assert_eq!(variants[2], EffectKind::BlobWrite);
    }

    #[test]
    fn effectrecord_returns_some_when_constructed_with_typical_components() {
        let record = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.stripe.com/v1/charges"}),
            crate::effects::EffectIntent::Prepared,
            None,
        );
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.intent_id(), "fx-123");
        assert_eq!(r.kind(), EffectKind::HttpCall);
        assert_eq!(
            r.params_json(),
            &json!({"url": "https://api.stripe.com/v1/charges"})
        );
        assert_eq!(r.status(), crate::effects::EffectIntent::Prepared);
        assert_eq!(r.committed_at(), None);
    }

    #[test]
    fn effectrecord_returns_some_when_constructed_with_single_char_intent_id() {
        let record = EffectRecord::new(
            "a".to_string(),
            EffectKind::SqlQuery,
            json!({"query": "SELECT 1"}),
            crate::effects::EffectIntent::Prepared,
            None,
        );
        assert!(record.is_some());
        assert_eq!(record.unwrap().intent_id(), "a");
    }

    #[test]
    fn effectrecord_returns_none_when_intent_id_is_empty() {
        let result = EffectRecord::new(
            "".to_string(),
            EffectKind::HttpCall,
            json!({}),
            crate::effects::EffectIntent::Prepared,
            None,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn effectrecord_returns_some_when_constructed_with_committed_status_and_timestamp() {
        let ts = crate::types::TimestampMs(1234);
        let record = EffectRecord::new(
            "fx-456".to_string(),
            EffectKind::BlobWrite,
            json!({"bucket": "my-bucket", "key": "obj"}),
            crate::effects::EffectIntent::Committed,
            Some(ts),
        );
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.status(), crate::effects::EffectIntent::Committed);
        assert_eq!(r.committed_at(), Some(&ts));
    }

    #[test]
    fn effectrecord_serializes_and_deserializes_via_json_round_trip() {
        let record = EffectRecord::new(
            "fx-789".to_string(),
            EffectKind::HttpCall,
            json!({"method": "POST", "url": "https://example.com"}),
            crate::effects::EffectIntent::Prepared,
            None,
        );
        let r = record.unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, r);
    }

    #[rstest]
    #[case(crate::effects::EffectIntent::Prepared, None)]
    #[case(
        crate::effects::EffectIntent::Committed,
        Some(crate::types::TimestampMs(1000))
    )]
    #[case(
        crate::effects::EffectIntent::RolledBack,
        Some(crate::types::TimestampMs(2000))
    )]
    fn effectrecord_compress_decompress_roundtrip_preserves_all_intents(
        #[case] status: crate::effects::EffectIntent,
        #[case] committed_at: Option<crate::types::TimestampMs>,
    ) {
        let record = EffectRecord::new(
            "fx-test-intent".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.example.com"}),
            status,
            committed_at,
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let decompressed = EffectRecord::decompress(&compressed).unwrap();
        assert_eq!(decompressed, record);
    }

    #[rstest]
    #[case(EffectKind::HttpCall)]
    #[case(EffectKind::SqlQuery)]
    #[case(EffectKind::BlobWrite)]
    fn effectrecord_compress_decompress_roundtrip_preserves_all_kinds(#[case] kind: EffectKind) {
        let record = EffectRecord::new(
            "fx-test-kind".to_string(),
            kind,
            json!({"query": "SELECT * FROM table"}),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let decompressed = EffectRecord::decompress(&compressed).unwrap();
        assert_eq!(decompressed, record);
    }

    #[test]
    fn effectrecord_compress_decompress_roundtrip_with_complex_params() {
        let record = EffectRecord::new(
            "fx-complex".to_string(),
            EffectKind::HttpCall,
            json!({
                "headers": {"Authorization": "Bearer token123", "Content-Type": "application/json"},
                "body": {"items": [{"id": 1, "name": "first"}, {"id": 2, "name": "second"}]},
                "timeout_ms": 5000
            }),
            crate::effects::EffectIntent::Committed,
            Some(crate::types::TimestampMs(9999)),
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let decompressed = EffectRecord::decompress(&compressed).unwrap();
        assert_eq!(decompressed, record);
    }

    #[test]
    fn effectrecord_compression_reduces_size_for_text_heavy_payload() {
        let record = EffectRecord::new(
            "fx-large".to_string(),
            EffectKind::HttpCall,
            json!({
                "body": "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100)
            }),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let json_bytes = serde_json::to_vec(&record).unwrap();
        let compressed = record.compress().unwrap();
        assert!(
            compressed.len() < json_bytes.len(),
            "Compressed size ({}) should be smaller than JSON size ({})",
            compressed.len(),
            json_bytes.len()
        );
    }

    #[test]
    fn effectrecord_compression_still_smaller_for_small_records() {
        let record = EffectRecord::new(
            "a".to_string(),
            EffectKind::SqlQuery,
            json!({"q": "x"}),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let json_bytes = serde_json::to_vec(&record).unwrap();
        let compressed = record.compress().unwrap();
        assert!(
            compressed.len() < json_bytes.len(),
            "Even small records should compress (compressed: {}, json: {})",
            compressed.len(),
            json_bytes.len()
        );
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_empty_input() {
        let result = EffectRecord::decompress(&[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_random_bytes() {
        let result = EffectRecord::decompress(b"not zstd compressed data at all");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_truncated_zstd() {
        let record = EffectRecord::new(
            "fx-trunc".to_string(),
            EffectKind::BlobWrite,
            json!({"bucket": "test"}),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let truncated = &compressed[..compressed.len() / 2];
        let result = EffectRecord::decompress(truncated);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_corrupted_zstd() {
        let mut corrupted = vec![0x28, 0xb5, 0x2f, 0xfd];
        corrupted.extend_from_slice(b"invalid zstd frame");
        let result = EffectRecord::decompress(&corrupted);
        assert!(result.is_err());
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_valid_zstd_invalid_utf8() {
        let valid_zstd_magic = [0x28, 0xb5, 0x2f, 0xfd];
        let result = EffectRecord::decompress(&valid_zstd_magic);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }

    #[test]
    fn test_validate_happy_path() {
        let effect = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.stripe.com/v1/charges", "amount": 100}),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let mut param_types = HashMap::new();
        param_types.insert("url".to_string(), JsonType::String);
        param_types.insert("amount".to_string(), JsonType::Number);
        let schema = StepSchema {
            expected_intent: crate::effects::EffectIntent::Prepared,
            expected_kind: EffectKind::HttpCall,
            required_params: vec!["url".to_string(), "amount".to_string()],
            param_types,
        };
        let result = validate_effect_against_schema(&effect, &schema);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_intent_mismatch() {
        let effect = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.stripe.com/v1/charges"}),
            crate::effects::EffectIntent::Committed,
            None,
        )
        .unwrap();
        let schema = StepSchema {
            expected_intent: crate::effects::EffectIntent::Prepared,
            expected_kind: EffectKind::HttpCall,
            required_params: vec![],
            param_types: HashMap::new(),
        };
        let result = validate_effect_against_schema(&effect, &schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectValidationError::IntentMismatch(
                crate::effects::EffectIntent::Committed,
                crate::effects::EffectIntent::Prepared
            )
        ));
    }

    #[test]
    fn test_validate_missing_required_param() {
        let effect = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::SqlQuery,
            json!({"table": "users"}),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let schema = StepSchema {
            expected_intent: crate::effects::EffectIntent::Prepared,
            expected_kind: EffectKind::SqlQuery,
            required_params: vec!["query".to_string()],
            param_types: HashMap::new(),
        };
        let result = validate_effect_against_schema(&effect, &schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectValidationError::MissingParam { name } if name == "query"
        ));
    }

    #[test]
    fn test_validate_type_mismatch() {
        let effect = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::HttpCall,
            json!({"count": "string_value"}),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let mut param_types = HashMap::new();
        param_types.insert("count".to_string(), JsonType::Number);
        let schema = StepSchema {
            expected_intent: crate::effects::EffectIntent::Prepared,
            expected_kind: EffectKind::HttpCall,
            required_params: vec![],
            param_types,
        };
        let result = validate_effect_against_schema(&effect, &schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectValidationError::TypeMismatch {
                param,
                expected: JsonType::Number,
                actual: JsonType::String,
            } if param == "count"
        ));
    }

    #[test]
    fn test_validate_null_param_allowed() {
        let effect = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::HttpCall,
            json!({"optional_field": null}),
            crate::effects::EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let mut param_types = HashMap::new();
        param_types.insert("optional_field".to_string(), JsonType::Null);
        let schema = StepSchema {
            expected_intent: crate::effects::EffectIntent::Prepared,
            expected_kind: EffectKind::HttpCall,
            required_params: vec![],
            param_types,
        };
        let result = validate_effect_against_schema(&effect, &schema);
        assert!(result.is_ok());
    }
}
