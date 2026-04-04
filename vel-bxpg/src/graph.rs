//! Graph output (CLI Integration) for `--graph` flag.

use std::io::Write;

use crate::error::GraphOutputError;
use crate::types::WorkflowDefinition;

/// Output the DAG as JSON to stdout if `--graph` flag is present.
///
/// This function MUST be called after `Dag::build()` succeeds.
///
/// # Errors
/// Returns `GraphOutputError::SerializationFailed` if JSON serialization fails.
/// Returns `GraphOutputError::StdoutUnavailable` if stdout cannot be written to.
#[must_use]
pub fn output_graph(workflow: &WorkflowDefinition) -> Result<(), GraphOutputError> {
    // Serialize to JSON
    let json =
        serde_json::to_string(workflow).map_err(|_| GraphOutputError::SerializationFailed)?;

    // Write to stdout
    std::io::stdout()
        .write_all(json.as_bytes())
        .map_err(|_| GraphOutputError::StdoutUnavailable)?;

    Ok(())
}
