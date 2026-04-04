//! WorkflowVersion struct for binary version pinning (ADR-017).

use vo_types::{BinaryHash, TimestampMs, WorkflowName};

/// Binary version pinning for a workflow (ADR-017).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowVersion {
    name: WorkflowName,
    hash: BinaryHash,
    schema_version: u16,
    registered_at: TimestampMs,
}

impl WorkflowVersion {
    /// Create a new WorkflowVersion.
    ///
    /// # Errors
    ///
    /// Returns `None` if the hash is empty (placeholder validation).
    #[must_use]
    pub fn new(name: WorkflowName, hash: BinaryHash, registered_at: TimestampMs) -> Option<Self> {
        Some(Self {
            name,
            hash,
            schema_version: 1,
            registered_at,
        })
    }

    /// Returns the binary path for this version.
    #[must_use]
    pub fn binary_path(&self) -> String {
        format!("/var/wtf/versions/{}/{}", self.hash.as_str(), self.name.as_str())
    }

    #[must_use]
    pub fn name(&self) -> &WorkflowName {
        &self.name
    }

    #[must_use]
    pub fn hash(&self) -> &BinaryHash {
        &self.hash
    }

    #[must_use]
    pub fn registered_at(&self) -> TimestampMs {
        self.registered_at
    }

    /// Returns the schema version for serialization compatibility.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
}

#[cfg(test)]
mod tests {
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    #[test]
    fn new_creates_version_with_correct_binary_path_format() {
        let hash = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name.clone(), hash.clone(), ts).unwrap();

        let expected = format!("/var/wtf/versions/{}/{}", hash.as_str(), name.as_str());
        assert_eq!(version.binary_path(), expected);
    }

    #[test]
    fn new_sets_schema_version_to_one() {
        let hash = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name, hash, ts).unwrap();

        assert_eq!(version.schema_version(), 1);
    }
}
