//! Budget allocation tracking for the saga.
//!
//! This module contains the in-memory [`BudgetManifest`] that tracks
//! the state of each budget reservation through the saga lifecycle.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::append::WriteClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SagaStatus {
    Staged,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaEntry {
    pub write_key: String,
    pub class: WriteClass,
    pub size_bytes: u64,
    pub status: SagaStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetManifest {
    entries: HashMap<String, SagaEntry>,
    version: u64,
}

impl BudgetManifest {
    /// Stage a new write entry in the manifest.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::AlreadyExists`] if a write entry with the same key already exists.
    pub fn stage(
        &mut self,
        write_key: String,
        class: WriteClass,
        size_bytes: u64,
    ) -> Result<(), SagaError> {
        if self.entries.contains_key(&write_key) {
            return Err(SagaError::AlreadyExists(write_key));
        }
        self.entries.insert(
            write_key.clone(),
            SagaEntry {
                write_key,
                class,
                size_bytes,
                status: SagaStatus::Staged,
            },
        );
        Ok(())
    }

    /// Commit a previously staged write entry.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::InvalidState`] if the entry is not in the `Staged` state.
    pub fn commit(&mut self, write_key: &str) -> Result<(), SagaError> {
        let entry = self
            .entries
            .get_mut(write_key)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status != SagaStatus::Staged {
            return Err(SagaError::InvalidState {
                key: write_key.to_string(),
                expected: SagaStatus::Staged,
                actual: entry.status,
            });
        }
        entry.status = SagaStatus::Committed;
        self.version += 1;
        Ok(())
    }

    /// Roll back a write entry.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::AlreadyRolledBack`] if the entry is already rolled back.
    pub fn rollback(&mut self, write_key: &str) -> Result<(), SagaError> {
        let entry = self
            .entries
            .get_mut(write_key)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status == SagaStatus::RolledBack {
            return Err(SagaError::AlreadyRolledBack(write_key.to_string()));
        }
        entry.status = SagaStatus::RolledBack;
        self.version += 1;
        Ok(())
    }

    #[must_use]
    pub fn get(&self, write_key: &str) -> Option<&SagaEntry> {
        self.entries.get(write_key)
    }

    pub fn staged_entries(&self) -> impl Iterator<Item = &SagaEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaStatus::Staged)
    }

    pub fn committed_entries(&self) -> impl Iterator<Item = &SagaEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaStatus::Committed)
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn recover_staged_as_rolled_back(&mut self) {
        for entry in self.entries.values_mut() {
            if entry.status == SagaStatus::Staged {
                entry.status = SagaStatus::RolledBack;
            }
        }
        self.version += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SagaError {
    #[error("saga entry already exists: {0}")]
    AlreadyExists(String),
    #[error("saga entry not found: {0}")]
    NotFound(String),
    #[error("saga entry already rolled back: {0}")]
    AlreadyRolledBack(String),
    #[error("invalid state for {key}: expected {expected:?}, got {actual:?}")]
    InvalidState {
        key: String,
        expected: SagaStatus,
        actual: SagaStatus,
    },
    #[error("budget reserve failed: {0}")]
    BudgetReserveFailed(String),
    #[error("storage error: {reason}")]
    Storage { reason: String },
    #[error("corrupt saga entry for key {key}: {reason}")]
    CorruptEntry { key: String, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_stage_and_commit() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();

        let entry = manifest.get("key1").unwrap();
        assert_eq!(entry.status, SagaStatus::Staged);

        manifest.commit("key1").unwrap();
        let entry = manifest.get("key1").unwrap();
        assert_eq!(entry.status, SagaStatus::Committed);
    }

    #[test]
    fn manifest_stage_and_rollback() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();

        manifest.rollback("key1").unwrap();
        let entry = manifest.get("key1").unwrap();
        assert_eq!(entry.status, SagaStatus::RolledBack);
    }

    #[test]
    fn manifest_recover_staged_as_rolled_back() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();
        manifest
            .stage("key2".to_string(), WriteClass::BulkBlob, 200)
            .unwrap();
        manifest.commit("key1").unwrap();

        manifest.recover_staged_as_rolled_back();

        assert_eq!(manifest.get("key1").unwrap().status, SagaStatus::Committed);
        assert_eq!(manifest.get("key2").unwrap().status, SagaStatus::RolledBack);
    }

    #[test]
    fn manifest_commit_non_existent_fails() {
        let mut manifest = BudgetManifest::default();
        let result = manifest.commit("nonexistent");
        assert!(matches!(result, Err(SagaError::NotFound(_))));
    }

    #[test]
    fn manifest_double_commit_fails() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();
        manifest.commit("key1").unwrap();
        let result = manifest.commit("key1");
        assert!(matches!(result, Err(SagaError::InvalidState { .. })));
    }
}