use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::workspace::workspace_index_error::WorkspaceIndexError;

const MAX_METADATA_KEY_LEN: usize = 128;
const MAX_METADATA_VALUE_LEN: usize = 4096;
const MAX_METADATA_ENTRIES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub entries: BTreeMap<String, String>,
}

impl WorkspaceMetadata {
    pub fn empty() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<(), WorkspaceIndexError> {
        if self.entries.len() > MAX_METADATA_ENTRIES {
            return Err(WorkspaceIndexError::TooManyMetadataEntries {
                max: MAX_METADATA_ENTRIES,
                actual: self.entries.len(),
            });
        }
        for (key, value) in &self.entries {
            if key.len() > MAX_METADATA_KEY_LEN {
                return Err(WorkspaceIndexError::MetadataKeyTooLong {
                    max_length: MAX_METADATA_KEY_LEN,
                    actual_length: key.len(),
                });
            }
            if value.len() > MAX_METADATA_VALUE_LEN {
                return Err(WorkspaceIndexError::MetadataValueTooLong {
                    max_length: MAX_METADATA_VALUE_LEN,
                    actual_length: value.len(),
                });
            }
        }
        Ok(())
    }
}

impl fmt::Display for WorkspaceMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkspaceMetadata({} entries)", self.entries.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tm_001_empty_metadata() {
        let meta = WorkspaceMetadata::empty();
        assert!(meta.entries.is_empty());
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn tm_002_single_entry() {
        let mut meta = WorkspaceMetadata::empty();
        meta.entries.insert("key".to_string(), "value".to_string());
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn tm_003_duplicate_keys_last_write_wins() {
        let mut meta = WorkspaceMetadata::empty();
        meta.entries.insert("key".to_string(), "v1".to_string());
        meta.entries.insert("key".to_string(), "v2".to_string());
        assert_eq!(meta.entries.len(), 1);
        assert_eq!(meta.entries.get("key").unwrap(), "v2");
    }

    #[test]
    fn tm_004_key_at_128_bytes_accepted() {
        let mut meta = WorkspaceMetadata::empty();
        let key = "x".repeat(128);
        meta.entries.insert(key, "v".to_string());
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn tm_005_key_at_129_bytes_rejected() {
        let mut meta = WorkspaceMetadata::empty();
        let key = "x".repeat(129);
        meta.entries.insert(key, "v".to_string());
        assert!(matches!(
            meta.validate(),
            Err(WorkspaceIndexError::MetadataKeyTooLong {
                max_length: 128,
                actual_length: 129
            })
        ));
    }

    #[test]
    fn tm_006_value_at_4096_bytes_accepted() {
        let mut meta = WorkspaceMetadata::empty();
        let val = "x".repeat(4096);
        meta.entries.insert("k".to_string(), val);
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn tm_007_value_at_4097_bytes_rejected() {
        let mut meta = WorkspaceMetadata::empty();
        let val = "x".repeat(4097);
        meta.entries.insert("k".to_string(), val);
        assert!(matches!(
            meta.validate(),
            Err(WorkspaceIndexError::MetadataValueTooLong {
                max_length: 4096,
                actual_length: 4097
            })
        ));
    }

    #[test]
    fn tm_008_64_entries_accepted() {
        let mut meta = WorkspaceMetadata::empty();
        for i in 0..64 {
            meta.entries.insert(format!("k{i}"), "v".to_string());
        }
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn tm_009_65_entries_rejected() {
        let mut meta = WorkspaceMetadata::empty();
        for i in 0..65 {
            meta.entries.insert(format!("k{i}"), "v".to_string());
        }
        assert!(matches!(
            meta.validate(),
            Err(WorkspaceIndexError::TooManyMetadataEntries {
                max: 64,
                actual: 65
            })
        ));
    }
}
