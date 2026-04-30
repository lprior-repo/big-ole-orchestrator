//! CommandEnvelope metadata methods.
//!
//! This module contains methods that operate on the envelope's version/metadata.

use super::CommandEnvelope;

impl CommandEnvelope {
    /// Returns `true` if the envelope version is supported.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.schema_version <= super::correlation::MAX_SUPPORTED_COMMAND_VERSION
    }
}
