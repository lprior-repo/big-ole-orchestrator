//! WorkflowVersion struct for binary version pinning (ADR-017).
//!
//! Also includes `VersionPinnedInstance` for per-instance hash pinning (tw-4y6h.15.5, tw-4y6h.15.6).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub fn new(
        name: WorkflowName,
        hash: BinaryHash,
        registered_at: TimestampMs,
    ) -> Result<Self, WorkflowVersionError> {
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

/// A workflow instance that has been pinned to a specific binary hash (ADR-017, ADR-027).
///
/// Once created, the instance always uses the same immutable binary path,
/// even if the workflow is redeployed with a different hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionPinnedInstance {
    /// Unique instance identifier
    instance_id: String,
    /// Workflow name this instance belongs to
    workflow_name: WorkflowName,
    /// Pinned binary hash set at instance creation time
    pinned_hash: BinaryHash,
    /// The immutable binary path derived from the pinned hash
    binary_path: String,
    /// When this instance was created
    created_at: TimestampMs,
}

impl VersionPinnedInstance {
    /// Create a new version-pinned instance.
    ///
    /// The binary path is derived from the pinned hash and workflow name,
    /// following the same convention as `WorkflowVersion`.
    ///
    /// # Errors
    ///
    /// Returns `HashTooShort` if the hash is shorter than 64 characters.
    pub fn new(
        instance_id: String,
        workflow_name: WorkflowName,
        hash: BinaryHash,
        created_at: TimestampMs,
    ) -> Result<Self, WorkflowVersionError> {
        if hash.as_str().len() < 64 {
            return Err(WorkflowVersionError::HashTooShort);
        }
        let binary_path = format!("/var/wtf/versions/{}/{}", hash.as_str(), workflow_name.as_str());
        Ok(Self {
            instance_id,
            workflow_name,
            pinned_hash: hash,
            binary_path,
            created_at,
        })
    }

    /// Returns the instance identifier.
    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Returns the workflow name.
    #[must_use]
    pub fn workflow_name(&self) -> &WorkflowName {
        &self.workflow_name
    }

    /// Returns the pinned binary hash.
    #[must_use]
    pub fn pinned_hash(&self) -> &BinaryHash {
        &self.pinned_hash
    }

    /// Returns the immutable binary path for this instance.
    #[must_use]
    pub fn binary_path(&self) -> &str {
        &self.binary_path
    }

    /// Returns when this instance was created.
    #[must_use]
    pub fn created_at(&self) -> TimestampMs {
        self.created_at
    }
}

/// Registry of workflow versions with active hash tracking (ADR-017, ADR-027).
///
/// Manages the mapping from workflow name → active binary hash.
/// Supports redeployment: when a new hash is published, it becomes the active hash,
/// but previously created pinned instances continue using their original hash.
#[derive(Debug, Clone, Default)]
pub struct WorkflowVersionRegistry {
    /// workflow_name → active (latest published) hash
    active_versions: HashMap<WorkflowName, BinaryHash>,
}

impl WorkflowVersionRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_versions: HashMap::new(),
        }
    }

    /// Register a new workflow version, making it the active version for the workflow.
    ///
    /// This is how redeployment works: the new hash becomes active.
    /// Previously pinned instances are unaffected.
    ///
    /// # Errors
    ///
    /// Returns `HashTooShort` if the hash is shorter than 64 characters.
    pub fn register(
        &mut self,
        name: WorkflowName,
        hash: BinaryHash,
    ) -> Result<(), WorkflowVersionError> {
        if hash.as_str().len() < 64 {
            return Err(WorkflowVersionError::HashTooShort);
        }
        self.active_versions.insert(name.clone(), hash);
        Ok(())
    }

    /// Returns the currently active hash for a workflow, if any.
    #[must_use]
    pub fn active_hash(&self, name: &WorkflowName) -> Option<&BinaryHash> {
        self.active_versions.get(name)
    }

    /// Create a version-pinned instance using the currently active hash for the workflow.
    ///
    /// # Errors
    ///
    /// Returns `NoActiveVersion` if the workflow has no registered active version.
    pub fn pin_instance(
        &self,
        instance_id: String,
        workflow_name: &WorkflowName,
        created_at: TimestampMs,
    ) -> Result<VersionPinnedInstance, VersionPinError> {
        let hash = self
            .active_versions
            .get(workflow_name)
            .ok_or(VersionPinError::NoActiveVersion {
                workflow_name: workflow_name.as_str().to_string(),
            })?;

        VersionPinnedInstance::new(
            instance_id,
            workflow_name.clone(),
            hash.clone(),
            created_at,
        )
        .map_err(|_| VersionPinError::HashTooShort)
    }

    /// Returns true if the workflow has an active version registered.
    #[must_use]
    pub fn has_active_version(&self, name: &WorkflowName) -> bool {
        self.active_versions.contains_key(name)
    }
}

/// Errors from version pinning operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionPinError {
    /// Workflow has no active version registered.
    NoActiveVersion {
        workflow_name: String,
    },
    /// Hash is shorter than 64 hex characters.
    HashTooShort,
}

impl std::fmt::Display for VersionPinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoActiveVersion { workflow_name } => {
                write!(f, "No active version for workflow: {workflow_name}")
            }
            Self::HashTooShort => write!(f, "Hash is shorter than 64 hex characters"),
        }
    }
}

impl std::error::Error for VersionPinError {}

#[cfg(test)]
mod tests {
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    #[test]
    fn new_creates_version_with_correct_binary_path_format() {
        let hash =
            BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name.clone(), hash.clone(), ts).unwrap();

        let expected = format!("/var/wtf/versions/{}/{}", hash.as_str(), name.as_str());
        assert_eq!(version.binary_path(), expected);
    }

    #[test]
    fn new_sets_schema_version_to_one() {
        let hash =
            BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name, hash, ts).unwrap();

        assert_eq!(version.schema_version(), 1);
    }

    #[test]
    fn serde_roundtrip_preserves_all_fields() {
        let hash =
            BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let original = super::WorkflowVersion::new(name.clone(), hash.clone(), ts).unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: super::WorkflowVersion = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, original);
        assert!(json.contains("\"schema_version\""));
        assert!(
            json.contains("\"workflow_name\""),
            "JSON must use workflow_name field: {json}"
        );
        assert!(
            json.contains("\"version_hash\""),
            "JSON must use version_hash field: {json}"
        );
    }

    #[test]
    fn new_rejects_hash_shorter_than_64_chars() {
        let hash = BinaryHash::parse("aabbccdd").unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let result = super::WorkflowVersion::new(name, hash, ts);

        assert_eq!(result, Err(super::WorkflowVersionError::HashTooShort));
    }

    #[test]
    fn binary_path_returns_string_starting_with_prefix() {
        let hash =
            BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name, hash, ts).unwrap();

        let path = version.binary_path();
        assert!(path.starts_with("/var/wtf/versions/"));
    }

    #[test]
    fn binary_path_returns_str_ref_not_string() {
        let hash =
            BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let version = super::WorkflowVersion::new(name, hash, ts).unwrap();

        // Proves binary_path() returns &str (borrowed), not String (owned).
        let _path: &str = version.binary_path();
    }

    #[test]
    fn workflow_version_is_hashable_for_use_in_hashset() {
        use std::collections::HashSet;

        let hash =
            BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();
        let name = WorkflowName::parse("my-workflow").unwrap();
        let ts = TimestampMs::try_from(1712200000000u64).unwrap();

        let v = super::WorkflowVersion::new(name, hash, ts).unwrap();

        let mut set = HashSet::new();
        set.insert(v);
        assert_eq!(set.len(), 1);
    }
}
