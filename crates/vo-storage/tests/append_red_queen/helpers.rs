//! Test helpers for append_red_queen
//!
//! Shared utilities for generating test events and writes.

use vo_storage::append::{
    AppendEntry, ControlPlaneWrite, ProjectionWrite, BlobWrite, WriteBudget,
};
use vo_types::events::EventEnvelope;
#[cfg(test)]
use vo_types::events::EventMetadata;

pub fn make_event(instance_id: &str, sequence: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 + sequence,
        payload: serde_json::json!({ "seq": sequence }),
        metadata: EventMetadata::default(),
    }
}

pub fn make_control_plane_write(size_bytes: u64) -> ControlPlaneWrite {
    ControlPlaneWrite::new(make_event("inst-1", 1), size_bytes)
}

pub fn make_projection_write(id: &str, size_bytes: u64) -> ProjectionWrite {
    ProjectionWrite::new(id.to_string(), size_bytes)
}

pub fn make_blob_write(id: &str, size_bytes: u64) -> BlobWrite {
    BlobWrite::bulk(id.to_string(), size_bytes)
}
