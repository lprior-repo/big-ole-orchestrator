use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{BinaryHash, ParseError};

pub const VERSION_BASE_PATH: &str = "/var/wtf/versions";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscoveryPath {
    version_root: String,
    binary_hash: BinaryHash,
    binary_name: String,
}

impl DiscoveryPath {
    pub fn new(version_root: String, binary_hash: BinaryHash, binary_name: String) -> Self {
        Self {
            version_root,
            binary_hash,
            binary_name,
        }
    }

    pub fn version_root(&self) -> &str {
        &self.version_root
    }

    pub fn binary_hash(&self) -> &BinaryHash {
        &self.binary_hash
    }

    pub fn binary_name(&self) -> &str {
        &self.binary_name
    }

    pub fn parse(s: &str) -> Result<Self, DiscoveryPathError> {
        let s = s.strip_prefix("file://").unwrap_or(s);

        let expected_prefix = format!("{}/", VERSION_BASE_PATH);
        let remaining =
            s.strip_prefix(&expected_prefix)
                .ok_or_else(|| DiscoveryPathError::InvalidFormat {
                    reason: format!("path must start with {}/", VERSION_BASE_PATH),
                })?;

        let parts: Vec<&str> = remaining.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(DiscoveryPathError::InvalidFormat {
                reason: "path must be {}/<hash>/<binary_name>".to_string(),
            });
        }

        let hash_str = parts[0];
        let binary_name = parts[1].to_string();

        let binary_hash = BinaryHash::parse(hash_str).map_err(DiscoveryPathError::InvalidHash)?;

        Ok(Self {
            version_root: VERSION_BASE_PATH.to_string(),
            binary_hash,
            binary_name,
        })
    }

    pub fn to_string_lossy(&self) -> String {
        format!(
            "{}/{}/{}",
            self.version_root, self.binary_hash, self.binary_name
        )
    }

    pub fn with_binary_name(&self, name: String) -> Self {
        Self {
            version_root: self.version_root.clone(),
            binary_hash: self.binary_hash.clone(),
            binary_name: name,
        }
    }

    pub fn with_hash(&self, hash: BinaryHash) -> Self {
        Self {
            version_root: self.version_root.clone(),
            binary_hash: hash,
            binary_name: self.binary_name.clone(),
        }
    }
}

impl fmt::Display for DiscoveryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}",
            self.version_root, self.binary_hash, self.binary_name
        )
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DiscoveryPathError {
    #[error("invalid discovery path format: {reason}")]
    InvalidFormat { reason: String },

    #[error("invalid binary hash: {0}")]
    InvalidHash(ParseError),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionPin {
    pin_hash: BinaryHash,
    pinned_at_ms: u64,
}

impl VersionPin {
    pub fn new(pin_hash: BinaryHash, pinned_at_ms: u64) -> Self {
        Self {
            pin_hash,
            pinned_at_ms,
        }
    }

    pub fn pin_hash(&self) -> &BinaryHash {
        &self.pin_hash
    }

    pub fn pinned_at_ms(&self) -> u64 {
        self.pinned_at_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersionConstraint {
    Exact,
    Compatible,
    Latest,
}

impl VersionConstraint {
    pub fn matches(&self, candidate: &BinaryHash, pinned: &BinaryHash) -> bool {
        match self {
            VersionConstraint::Exact => candidate == pinned,
            VersionConstraint::Compatible => candidate.as_str()[..8] == pinned.as_str()[..8],
            VersionConstraint::Latest => true,
        }
    }
}

pub fn validate_discovery_path(path: &DiscoveryPath) -> Result<(), DiscoveryPathError> {
    if path.binary_name.is_empty() {
        return Err(DiscoveryPathError::InvalidFormat {
            reason: "binary_name cannot be empty".to_string(),
        });
    }
    if path.binary_name.contains('/') {
        return Err(DiscoveryPathError::InvalidFormat {
            reason: "binary_name cannot contain path separators".to_string(),
        });
    }
    Ok(())
}

pub fn enforce_pin(pin: &VersionPin, candidate: &BinaryHash) -> Result<(), PinEnforcementError> {
    if &pin.pin_hash != candidate {
        return Err(PinEnforcementError::HashMismatch {
            expected: pin.pin_hash.clone(),
            actual: candidate.clone(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PinEnforcementError {
    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch {
        expected: BinaryHash,
        actual: BinaryHash,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_path_parse_valid_path() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.binary_hash(),
            &BinaryHash::parse("abcdef0123456789").unwrap()
        );
        assert_eq!(path.binary_name(), "my-binary");
        assert_eq!(path.version_root(), VERSION_BASE_PATH);
    }

    #[test]
    fn discovery_path_parse_with_file_prefix() {
        let path =
            DiscoveryPath::parse("file:///var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.binary_hash(),
            &BinaryHash::parse("abcdef0123456789").unwrap()
        );
        assert_eq!(path.binary_name(), "my-binary");
    }

    #[test]
    fn discovery_path_parse_invalid_prefix() {
        let result = DiscoveryPath::parse("/other/path/abcdef0123456789/binary");
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn discovery_path_parse_invalid_hash() {
        let result = DiscoveryPath::parse("/var/wtf/versions/notahext/binary");
        assert!(matches!(result, Err(DiscoveryPathError::InvalidHash(_))));
    }

    #[test]
    fn discovery_path_display() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.to_string(),
            "/var/wtf/versions/abcdef0123456789/my-binary"
        );
    }

    #[test]
    fn version_constraint_exact_matches() {
        let hash = BinaryHash::parse("abcdef0123456789").unwrap();
        let constraint = VersionConstraint::Exact;
        assert!(constraint.matches(&hash, &hash));
    }

    #[test]
    fn version_constraint_exact_no_match() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Exact;
        assert!(!constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn version_constraint_compatible_same_prefix() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("abcdef01deadbeef").unwrap();
        let constraint = VersionConstraint::Compatible;
        assert!(constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn version_constraint_compatible_different_prefix() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Compatible;
        assert!(!constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn version_constraint_latest_always_matches() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Latest;
        assert!(constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn validate_discovery_path_empty_name() {
        let path = DiscoveryPath::new(
            VERSION_BASE_PATH.to_string(),
            BinaryHash::parse("abcdef0123456789").unwrap(),
            String::new(),
        );
        let result = validate_discovery_path(&path);
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn validate_discovery_path_name_with_separator() {
        let path = DiscoveryPath::new(
            VERSION_BASE_PATH.to_string(),
            BinaryHash::parse("abcdef0123456789").unwrap(),
            "foo/bar".to_string(),
        );
        let result = validate_discovery_path(&path);
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn enforce_pin_success() {
        let hash = BinaryHash::parse("abcdef0123456789").unwrap();
        let pin = VersionPin::new(hash.clone(), 1000);
        enforce_pin(&pin, &hash).unwrap();
    }

    #[test]
    fn enforce_pin_mismatch() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let pin = VersionPin::new(hash1, 1000);
        let result = enforce_pin(&pin, &hash2);
        assert!(matches!(
            result,
            Err(PinEnforcementError::HashMismatch { .. })
        ));
    }
}
