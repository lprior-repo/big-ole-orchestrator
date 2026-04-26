//! Durable execution receipt for committed connector effects (ADR-041 §4).

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Receipt {
    effect_id: String,
    connector_type: String,
    connector_version: String,
    external_receipt: serde_json::Value,
    committed_at: crate::types::TimestampMs,
}

impl Receipt {
    /// Construct a new Receipt. Returns None if effect_id or connector_type is empty.
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
