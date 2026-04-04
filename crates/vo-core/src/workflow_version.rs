//! WorkflowVersion struct for binary version pinning (ADR-017).

use serde::{Deserialize, Serialize};
use vo_types::{BinaryHash, TimestampMs, WorkflowName};

/// Binary version pinning for a workflow (ADR-017).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowVersion {
    #[serde(rename = "workflow_name")]
    name: WorkflowName,
    #[serde(rename = "version_hash")]
    hash: BinaryHash,
    schema_version: u16,
    registered_at: TimestampMs,
    binary_path: String,
}

/// Errors from WorkflowVersion construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowVersionError {
    /// Hash is shorter than 64 hex characters.
    HashTooShort,
}

impl WorkflowVersion {
    /// Create a new WorkflowVersion.
    ///
    /// # Errors
    ///
    /// Returns an error if the hash is shorter than 64 characters.
    pub fn new(name: WorkflowName, hash: BinaryHash, registered_at: TimestampMs) -> Result<Self, WorkflowVersionError> {
        if hash.as_str().len() < 64 {
            return Err(WorkflowVersionError::HashTooShort);
        }
        let binary_path = format!("/var/wtf/versions/{}/{}", hash.as_str(), name.as_str());
        Ok(Self {
            name,
            hash,
            schema_version: 1,
            registered_at,
            binary_path,
        })
    }

    /// Returns the binary path for this version.
    #[must_use]
    pub fn binary_path(&self) -> &str {
        &self.binary_path
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

    #[test]
    fn serde_roundtrip_preserves_all_fields() {
        let hash = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let original = super::WorkflowVersion::new(name.clone(), hash.clone(), ts).unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: super::WorkflowVersion = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, original);
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"workflow_name\""), "JSON must use workflow_name field: {json}");
        assert!(json.contains("\"version_hash\""), "JSON must use version_hash field: {json}");
    }

    #[test]
    fn new_rejects_hash_shorter_than_64_chars() {
        let hash = BinaryHash::parse("aabbccdd").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let result = super::WorkflowVersion::new(name, hash, ts);

        assert!(result.is_err());
    }

    #[test]
    fn binary_path_returns_string_starting_with_prefix() {
        let hash = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name, hash, ts).unwrap();

        let path = version.binary_path();
        assert!(path.starts_with("/var/wtf/versions/"));
    }

    #[test]
    fn binary_path_returns_str_ref_not_string() {
        let hash = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name, hash, ts).unwrap();

        // Proves binary_path() returns &str (borrowed), not String (owned).
        let _path: &str = version.binary_path();
    }

    #[test]
    fn workflow_version_is_hashable_for_use_in_hashset() {
        use std::collections::HashSet;

        let hash = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let v = super::WorkflowVersion::new(name, hash, ts).unwrap();

        let mut set = HashSet::new();
        set.insert(v);
        assert_eq!(set.len(), 1);
    }
}
