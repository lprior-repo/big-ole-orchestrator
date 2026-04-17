//! Event metadata types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::events::error::Error;
use crate::CommandMetadata;

/// Typed metadata wrapper replacing the previous serde_json::Value metadata field.
/// Carries optional command provenance and room for future annotation keys.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EventMetadata {
    #[serde(default)]
    pub command_metadata: Option<CommandMetadata>,
    #[serde(default)]
    pub annotations: HashMap<String, serde_json::Value>,
}

impl EventMetadata {
    /// Deserialize from a JSON value (object).
    pub fn from_json(value: &serde_json::Value) -> Result<Self, Error> {
        serde_json::from_value(value.clone()).map_err(|e| {
            if e.to_string().contains("unknown variant") {
                // Try to extract the unknown variant name
                let err_str = e.to_string();
                if let Some(start) = err_str.find('`') {
                    if let Some(end) = err_str[start + 1..].find('`') {
                        let unknown_variant = &err_str[start + 1..start + 1 + end];
                        return Error::InvalidIssuer(unknown_variant.to_string());
                    }
                }
            }
            Error::InvalidCommandMetadata
        })
    }

    /// Serialize to a JSON object value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("EventMetadata should always serialize")
    }
}
