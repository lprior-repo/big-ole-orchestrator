//! Top-level event decoding (envelope + payload).

use crate::events::envelope::EventEnvelope;
use crate::events::error::Error;
use crate::events::payload::EventPayload;

/// Decode a full event (envelope + payload) from raw bytes.
///
/// # Errors
///
/// Returns `Error::PayloadDecodeSkipped` if the envelope version is unsupported,
/// `Error::EnvelopeDecodeFailed` on envelope parse failures, or
/// `Error::PayloadDecodeFailed` on payload parse failures.
pub fn decode_event(input: &[u8]) -> Result<(EventEnvelope, EventPayload), Error> {
    let envelope = match EventEnvelope::from_bytes(input) {
        Err(Error::UnsupportedEnvelopeVersion(_)) => {
            return Err(Error::PayloadDecodeSkipped);
        }
        Err(e) => {
            return Err(Error::EnvelopeDecodeFailed {
                source: Box::new(e),
            });
        }
        Ok(envelope) => envelope,
    };
    if !envelope.is_supported() {
        return Err(Error::PayloadDecodeSkipped);
    }
    let payload =
        EventPayload::try_from_json(&envelope.payload).map_err(|e| Error::PayloadDecodeFailed {
            source: Box::new(e),
        })?;
    Ok((envelope, payload))
}
