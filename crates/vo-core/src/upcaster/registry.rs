//! Upcaster registry for resolving and applying upcaster chains.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::upcaster::{EventEnvelopeError, Upcaster, UpcasterError, MAX_SUPPORTED_VERSION};
use vo_types::events::EventEnvelope;

/// Registry for resolving and applying upcaster chains
pub trait UpcasterRegistry: Send + Sync {
    /// Register an upcaster for a specific source version
    fn register(&self, upcaster: Box<dyn Upcaster>) -> Result<(), UpcasterError>;

    /// Resolve the upcaster chain and apply all necessary transforms
    fn upcast_envelope(&self, envelope: EventEnvelope) -> Result<EventEnvelope, UpcasterError>;

    /// Get the current maximum supported version
    fn max_supported_version(&self) -> u8;
}

/// Builder for constructing an upcaster registry
pub trait UpcasterRegistryBuilder: Send + Sync {
    fn build() -> Box<dyn UpcasterRegistry>;
}

// =============================================================================
// Concrete Implementation: UpcasterRegistryImpl
// =============================================================================

/// Concrete implementation of UpcasterRegistry for production use.
pub struct UpcasterRegistryImpl {
    upcasters: Arc<Mutex<HashMap<u8, Box<dyn Upcaster>>>>,
    max_version: u8,
}

impl UpcasterRegistryImpl {
    /// Creates a new UpcasterRegistryImpl with the given maximum supported version.
    #[must_use]
    pub fn new(max_version: u8) -> Self {
        Self {
            upcasters: Arc::new(Mutex::new(HashMap::new())),
            max_version,
        }
    }
}

impl UpcasterRegistry for UpcasterRegistryImpl {
    /// Register an upcaster for a specific source version.
    ///
    /// # Errors
    ///
    /// Returns `UpcasterError::InvalidTargetVersion` if the source version exceeds
    /// the maximum supported version.
    ///
    /// Returns `UpcasterError::DuplicateRegistration` if an upcaster is already
    /// registered for the given source version.
    fn register(&self, upcaster: Box<dyn Upcaster>) -> Result<(), UpcasterError> {
        let source_version = upcaster.source_version();
        let target_version = upcaster.target_version();

        if target_version > self.max_version {
            return Err(UpcasterError::InvalidTargetVersion(target_version));
        }

        let mut upcasters = self
            .upcasters
            .lock()
            .map_err(|_| UpcasterError::UpcastingFailed("lock poisoned".to_string()))?;

        if upcasters.contains_key(&source_version) {
            return Err(UpcasterError::DuplicateRegistration(source_version));
        }

        upcasters.insert(source_version, upcaster);
        Ok(())
    }

    /// Resolve the upcaster chain and apply all necessary transforms.
    ///
    /// # Errors
    ///
    /// Returns `UpcasterError::NoUpcasterRegistered` if no upcaster is registered
    /// for the current envelope version.
    ///
    /// Returns `UpcasterError::InvalidTargetVersion` if an upcaster produces a
    /// version exceeding the maximum supported version.
    ///
    /// Returns `UpcasterError::CircularChain` if the upcasters form a cycle.
    ///
    /// Returns `UpcasterError::InvalidUpcastedEnvelope` if an upcaster produces
    /// output that cannot be parsed as a valid envelope.
    fn upcast_envelope(&self, envelope: EventEnvelope) -> Result<EventEnvelope, UpcasterError> {
        // If already at or above max version, return unchanged
        if envelope.schema_version >= self.max_version {
            return Ok(envelope);
        }

        let upcasters = self
            .upcasters
            .lock()
            .map_err(|_| UpcasterError::UpcastingFailed("lock poisoned".to_string()))?;

        let chain = build_upcast_chain(&upcasters, envelope.schema_version, self.max_version)?;
        apply_upcast_chain(&envelope, self.max_version, chain)
    }

    /// Get the current maximum supported version.
    fn max_supported_version(&self) -> u8 {
        self.max_version
    }
}

/// Builds the ordered chain of upcasters needed to reach max_version.
///
/// Returns the sequence of (source_version, upcaster) pairs to apply.
type UpcasterChain<'a> = Vec<(u8, &'a dyn Upcaster)>;

fn build_upcast_chain(
    upcasters: &HashMap<u8, Box<dyn Upcaster>>,
    start_version: u8,
    max_version: u8,
) -> Result<UpcasterChain<'_>, UpcasterError> {
    let mut chain = Vec::new();
    let mut visited = HashMap::new();
    let mut current_version = start_version;

    while current_version < max_version {
        check_circular_chain(current_version, &visited)?;
        let upcaster = get_upcaster_for_version(upcasters, current_version)?;
        chain.push((current_version, upcaster));
        visited.insert(current_version, true);
        current_version = upcaster.target_version();
    }

    Ok(chain)
}

/// Checks for circular upcaster chain, returning error if version was already visited.
fn check_circular_chain(version: u8, visited: &HashMap<u8, bool>) -> Result<(), UpcasterError> {
    if visited.get(&version) == Some(&true) {
        Err(UpcasterError::CircularChain(version))
    } else {
        Ok(())
    }
}

/// Retrieves the upcaster for the given version, or error if none registered.
fn get_upcaster_for_version(
    upcasters: &HashMap<u8, Box<dyn Upcaster>>,
    version: u8,
) -> Result<&dyn Upcaster, UpcasterError> {
    upcasters
        .get(&version)
        .ok_or(UpcasterError::NoUpcasterRegistered(version))
        .map(|b| &**b as &dyn Upcaster)
}

/// Applies a single upcaster step, returning new version and payload.
fn apply_single_upcast(
    envelope: &EventEnvelope,
    version: u8,
    upcaster: &dyn Upcaster,
    current_payload: &serde_json::Value,
    max_version: u8,
) -> Result<(u8, serde_json::Value), UpcasterError> {
    let input_bytes = serialize_envelope_for_upcast(envelope, version, current_payload)?;
    let output_bytes = upcaster.upcast(&input_bytes)?;
    parse_and_validate_upcasted_envelope(&output_bytes, max_version)
}

/// Applies the upcast chain to transform the envelope payload.
fn apply_upcast_chain<'a>(
    envelope: &EventEnvelope,
    max_version: u8,
    chain: UpcasterChain<'a>,
) -> Result<EventEnvelope, UpcasterError> {
    let mut current_version = envelope.schema_version;
    let mut current_payload = envelope.payload.clone();

    for (version, upcaster) in chain {
        let (new_version, new_payload) =
            apply_single_upcast(envelope, version, upcaster, &current_payload, max_version)?;
        current_version = new_version;
        current_payload = new_payload;
    }

    Ok(EventEnvelope {
        schema_version: current_version,
        instance_id: envelope.instance_id.clone(),
        sequence: envelope.sequence,
        timestamp_ms: envelope.timestamp_ms,
        payload: current_payload,
        metadata: envelope.metadata.clone(),
    })
}

/// Serializes the current envelope state as JSON for upcaster input.
fn serialize_envelope_for_upcast(
    envelope: &EventEnvelope,
    version: u8,
    payload: &serde_json::Value,
) -> Result<Vec<u8>, UpcasterError> {
    let mut envelope_json = serde_json::Map::new();
    envelope_json.insert("version".to_string(), serde_json::json!(version));
    envelope_json.insert(
        "instance_id".to_string(),
        serde_json::json!(envelope.instance_id.clone()),
    );
    envelope_json.insert("sequence".to_string(), serde_json::json!(envelope.sequence));
    envelope_json.insert(
        "timestamp_ms".to_string(),
        serde_json::json!(envelope.timestamp_ms),
    );
    envelope_json.insert("payload".to_string(), payload.clone());
    envelope_json.insert("metadata".to_string(), envelope.metadata.to_json());

    serde_json::to_vec(&envelope_json)
        .map_err(|e| UpcasterError::UpcastingFailed(format!("serialization error: {}", e)))
}

/// Extracts and validates version number from envelope object.
fn extract_version(
    output_obj: &serde_json::Map<String, serde_json::Value>,
    max_version: u8,
) -> Result<u8, UpcasterError> {
    let v = parse_version_from_json(output_obj)?;
    validate_version_within_limit(v, max_version)
}

/// Parses the version field from JSON, returning u8 or error.
fn parse_version_from_json(
    output_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<u8, UpcasterError> {
    output_obj
        .get("version")
        .and_then(|v| v.as_u64())
        .map(u8::try_from)
        .ok_or_else(|| {
            UpcasterError::InvalidUpcastedEnvelope(EventEnvelopeError::MissingEnvelopeField(
                "version".to_string(),
            ))
        })?
        .map_err(|_| {
            UpcasterError::InvalidUpcastedEnvelope(EventEnvelopeError::InvalidEnvelopeField(
                "version exceeds u8 range".to_string(),
            ))
        })
}

/// Validates that version does not exceed max_version.
fn validate_version_within_limit(v: u8, max_version: u8) -> Result<u8, UpcasterError> {
    if v > max_version {
        Err(UpcasterError::InvalidTargetVersion(v))
    } else {
        Ok(v)
    }
}

/// Parses and validates upcaster output, extracting new version and payload.
fn parse_and_validate_upcasted_envelope(
    output_bytes: &[u8],
    max_version: u8,
) -> Result<(u8, serde_json::Value), UpcasterError> {
    let output_json: serde_json::Value = serde_json::from_slice(output_bytes).map_err(|_| {
        UpcasterError::InvalidUpcastedEnvelope(EventEnvelopeError::InvalidEnvelopeFormat)
    })?;

    let output_obj = output_json.as_object().ok_or({
        UpcasterError::InvalidUpcastedEnvelope(EventEnvelopeError::InvalidEnvelopeFormat)
    })?;

    let new_version = extract_version(output_obj, max_version)?;
    let new_payload = output_obj.get("payload").cloned().ok_or_else(|| {
        UpcasterError::InvalidUpcastedEnvelope(EventEnvelopeError::MissingEnvelopeField(
            "payload".to_string(),
        ))
    })?;

    Ok((new_version, new_payload))
}

// =============================================================================
// Builder Implementation: DefaultUpcasterRegistryBuilder
// =============================================================================

/// Builder for constructing an UpcasterRegistryImpl.
pub struct DefaultUpcasterRegistryBuilder;

impl UpcasterRegistryBuilder for DefaultUpcasterRegistryBuilder {
    /// Build a new UpcasterRegistryImpl with MAX_SUPPORTED_VERSION as max version.
    fn build() -> Box<dyn UpcasterRegistry> {
        Box::new(UpcasterRegistryImpl::new(MAX_SUPPORTED_VERSION))
    }
}
