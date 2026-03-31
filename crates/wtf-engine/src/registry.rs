use std::fs;

use dashmap::DashMap;
use wtf_types::{BinaryHash, WorkflowDefinition, WorkflowName};

use crate::error::BinaryRegistryError;
use crate::helpers::{compute_binary_hash, copy_to_versions, discover_graph, verify_source};
use crate::types::{BinaryPath, ReaperReport, RegistrationStatus, WorkflowRegistration};

/// Live, concurrent registry backed by DashMap.
/// Safe for concurrent reads from multiple actor threads.
pub struct BinaryRegistry {
    pub(crate) inner: DashMap<WorkflowName, WorkflowRegistration>,
    versions_dir: BinaryPath,
}

impl BinaryRegistry {
    /// Create a new empty registry.
    pub fn new(versions_dir: BinaryPath) -> Result<Self, BinaryRegistryError> {
        Ok(Self {
            inner: DashMap::new(),
            versions_dir,
        })
    }

    /// Register a binary: hash, copy to versions dir, discover graph, store registration.
    pub fn register(
        &self,
        source_path: &BinaryPath,
        workflow_name: WorkflowName,
    ) -> Result<(), BinaryRegistryError> {
        let source = source_path.as_path();
        verify_source(source, source_path)?;

        let (_, hex_hash) = compute_binary_hash(source, source_path)?;
        let binary_hash =
            BinaryHash::parse(&hex_hash).expect("SHA-256 hex is always a valid BinaryHash");

        let (versioned_binary_path, did_copy) =
            copy_to_versions(source, &self.versions_dir, &hex_hash, source_path)?;

        match discover_graph(versioned_binary_path.as_path(), &workflow_name) {
            Ok(definition) => {
                self.inner.insert(
                    workflow_name.clone(),
                    WorkflowRegistration {
                        workflow_name,
                        versioned_path: versioned_binary_path,
                        binary_hash,
                        status: RegistrationStatus::Active,
                        definition,
                    },
                );
                Ok(())
            }
            Err(e) => {
                if did_copy {
                    if let Err(cleanup_err) = fs::remove_file(versioned_binary_path.as_path()) {
                        eprintln!(
                            "failed to clean up versioned binary at {}: {cleanup_err}",
                            versioned_binary_path.as_path().display()
                        );
                    }
                }
                Err(e)
            }
        }
    }

    /// Resolve a workflow to its versioned binary path, hash, and definition.
    pub fn resolve(
        &self,
        workflow_name: &WorkflowName,
    ) -> Result<(BinaryPath, BinaryHash, WorkflowDefinition), BinaryRegistryError> {
        match self.inner.get(workflow_name) {
            None => Err(BinaryRegistryError::NotFound {
                workflow_name: workflow_name.clone(),
            }),
            Some(entry) => match entry.status {
                RegistrationStatus::Active => Ok((
                    entry.versioned_path.clone(),
                    entry.binary_hash.clone(),
                    entry.definition.clone(),
                )),
                RegistrationStatus::Deactivated => Err(BinaryRegistryError::WorkflowDeactivated {
                    workflow_name: workflow_name.clone(),
                }),
            },
        }
    }

    /// Deactivate a workflow (transition Active -> Deactivated).
    pub fn deactivate(&self, workflow_name: &WorkflowName) -> Result<(), BinaryRegistryError> {
        match self.inner.get_mut(workflow_name) {
            None => Err(BinaryRegistryError::NotFound {
                workflow_name: workflow_name.clone(),
            }),
            Some(mut entry) => {
                entry.status = RegistrationStatus::Deactivated;
                Ok(())
            }
        }
    }

    /// Run one Reaper GC sweep. The `has_active` closure returns true if a given
    /// workflow has active instances and should be skipped.
    pub fn reap<F>(&self, has_active: F) -> ReaperReport
    where
        F: Fn(&WorkflowName) -> bool,
    {
        let mut report = ReaperReport::default();
        let candidates = self.find_deactivated(&has_active, &mut report);
        let to_remove = Self::delete_versioned_binaries(candidates, &mut report);
        for name in to_remove {
            self.inner.remove(&name);
        }
        report
    }

    fn find_deactivated<F>(
        &self,
        has_active: &F,
        report: &mut ReaperReport,
    ) -> Vec<(WorkflowName, BinaryPath)>
    where
        F: Fn(&WorkflowName) -> bool,
    {
        let mut candidates = Vec::new();
        for entry in self.inner.iter() {
            if let RegistrationStatus::Deactivated = entry.value().status {
                if has_active(entry.key()) {
                    report.skipped.push(entry.key().clone());
                } else {
                    candidates.push((entry.key().clone(), entry.value().versioned_path.clone()));
                }
            }
        }
        candidates
    }

    fn delete_versioned_binaries(
        candidates: Vec<(WorkflowName, BinaryPath)>,
        report: &mut ReaperReport,
    ) -> Vec<WorkflowName> {
        let mut to_remove = Vec::new();
        for (name, path) in candidates {
            match fs::remove_file(path.as_path()) {
                Ok(()) => {
                    to_remove.push(name.clone());
                    report.reaped.push(name);
                }
                Err(e) => {
                    report.failures.push((
                        name,
                        BinaryRegistryError::ReaperDeleteFailed { path, source: e },
                    ));
                }
            }
        }
        to_remove
    }

    /// List all registered workflows.
    pub fn list(&self) -> Vec<(WorkflowName, WorkflowRegistration)> {
        self.inner
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect()
    }

    /// Get the number of registered workflows.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
