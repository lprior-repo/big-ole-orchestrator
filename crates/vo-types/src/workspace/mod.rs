mod workspace_id;
mod workspace_index;
mod workspace_index_error;
mod workspace_metadata;
mod workspace_name;
mod workspace_node;
mod workspace_path;

pub use workspace_id::WorkspaceId;
pub use workspace_index::WorkspaceIndex;
pub use workspace_index_error::WorkspaceIndexError;
pub use workspace_metadata::WorkspaceMetadata;
pub use workspace_name::WorkspaceName;
pub use workspace_node::WorkspaceNode;
pub use workspace_path::WorkspacePath;

#[cfg(test)]
mod workspace_edge_cases;
#[cfg(test)]
mod workspace_index_errors;
#[cfg(test)]
mod workspace_index_invariants;
#[cfg(test)]
mod workspace_index_lifecycle;
#[cfg(test)]
mod workspace_index_queries;
#[cfg(test)]
mod workspace_index_sequences;
#[cfg(test)]
mod workspace_index_snapshot;
#[cfg(test)]
mod workspace_serde;
