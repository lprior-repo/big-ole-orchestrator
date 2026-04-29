use crate::data::{FleetError, Rig};
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::warn;

#[derive(Debug, Deserialize)]
struct BeadsMetadata {
    backend: String,
    database: String,
    dolt_database: String,
    dolt_mode: String,
    dolt_server_host: String,
    dolt_server_port: Option<u16>,
}

pub async fn ensure_dolt_alive() -> Result<(), FleetError> {
    let check = Command::new("gt")
        .args(["dolt", "status"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(FleetError::Io)?;

    let stdout = String::from_utf8_lossy(&check.stdout);
    if check.status.success() && stdout.contains("Dolt server is running") {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&check.stderr);
    Err(FleetError::Bd(format!(
        "dolt unhealthy; status output: {}; stderr: {}",
        stdout.trim(),
        stderr.trim()
    )))
}

pub async fn guard_dolt(rig: &Rig, operation: &str) -> bool {
    match ensure_dolt_alive().await {
        Ok(()) => true,
        Err(error) => {
            warn!(
                "{}: skipping {} because Dolt is unhealthy: {}",
                rig.name, operation, error
            );
            false
        }
    }
}

pub async fn validate_rig_route(rig: &Rig) -> Result<(), FleetError> {
    let src_dir = Path::new(rig.src_dir);
    if !src_dir.is_dir() {
        return Err(FleetError::Config(format!(
            "source directory does not exist: {}",
            rig.src_dir
        )));
    }

    let metadata_path = src_dir.join(".beads/metadata.json");
    let metadata_json = tokio::fs::read_to_string(&metadata_path)
        .await
        .map_err(FleetError::Io)?;
    let metadata =
        serde_json::from_str::<BeadsMetadata>(&metadata_json).map_err(FleetError::Json)?;

    validate_metadata(rig, &metadata)
}

fn validate_metadata(rig: &Rig, metadata: &BeadsMetadata) -> Result<(), FleetError> {
    if metadata.backend != "dolt" || metadata.database != "dolt" || metadata.dolt_mode != "server" {
        return Err(FleetError::Config(format!(
            "{} metadata is not server-mode dolt",
            rig.name
        )));
    }

    if metadata.dolt_server_host != "127.0.0.1" {
        return Err(FleetError::Config(format!(
            "{} metadata points at unexpected host {}",
            rig.name, metadata.dolt_server_host
        )));
    }

    if metadata.dolt_database != rig.dolt_database {
        return Err(FleetError::Config(format!(
            "{} metadata database {} does not match configured {}",
            rig.name, metadata.dolt_database, rig.dolt_database
        )));
    }

    if metadata.dolt_server_port != Some(rig.dolt_port) {
        return Err(FleetError::Config(format!(
            "{} metadata port {:?} does not match configured {}",
            rig.name, metadata.dolt_server_port, rig.dolt_port
        )));
    }

    Ok(())
}
