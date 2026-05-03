use crate::data::{FleetError, FleetMetrics, ModuleMetrics, Rig};
use crate::review_beads::{build_review_bead, ReviewSkill};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

const DOLT_MUTATION_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(250);

fn metrics_path(rig: &Rig) -> PathBuf {
    PathBuf::from(format!("{}/{}/fleet-metrics.json", rig.gt_root, rig.name))
}

fn load_metrics(rig: &Rig) -> FleetMetrics {
    let path = metrics_path(rig);
    let Ok(data) = std::fs::read_to_string(&path) else {
        return FleetMetrics::default();
    };
    let Ok(metrics) = serde_json::from_str::<FleetMetrics>(&data) else {
        return FleetMetrics::default();
    };
    metrics
}

fn save_metrics(rig: &Rig, metrics: &FleetMetrics) {
    let path = metrics_path(rig);
    if let Ok(data) = serde_json::to_string_pretty(metrics) {
        let _ = std::fs::write(&path, data);
    }
}

async fn scan_modules(rig: &Rig) -> Result<Vec<String>, FleetError> {
    let output = Command::new("find")
        .args([
            rig.src_dir,
            "-name",
            "*.rs",
            "-not",
            "-path",
            "*/target/*",
            "-not",
            "-path",
            "*/.beads/*",
            "-not",
            "-path",
            "*/.git/*",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(FleetError::Io)?;

    let stdout = String::from_utf8(output.stdout).map_err(FleetError::Utf8)?;
    let prefix = format!("{}/", rig.src_dir);
    let modules: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix).map(String::from))
        .collect();

    Ok(modules)
}

fn select_target_modules(modules: &[String], metrics: &FleetMetrics, count: usize) -> Vec<String> {
    let mut scored: Vec<(u32, &String)> = modules
        .iter()
        .map(|module| {
            let beads = metrics
                .modules
                .iter()
                .find(|metric| metric.module == *module)
                .map_or(0, |metric| metric.beads_created);
            (beads, module)
        })
        .collect();

    scored.sort_by_key(|(count, _)| *count);
    scored
        .into_iter()
        .take(count)
        .map(|(_, module)| module.clone())
        .collect()
}

async fn create_review_bead(rig: &Rig, module: &str, skill: ReviewSkill) -> bool {
    let bead = build_review_bead(rig, module, skill);
    let result = Command::new("bd")
        .args([
            "create",
            &bead.title,
            "--description",
            &bead.description,
            "--acceptance",
            &bead.acceptance,
            "--design",
            &bead.design,
            "--type",
            "task",
            "-p",
            "2",
        ])
        .current_dir(rig.src_dir)
        .env("BD_DOLT_AUTO_COMMIT", "off")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    matches!(result, Ok(status) if status.success())
}

fn record_created_metric(metrics: &mut FleetMetrics, module: &str) {
    let entry = metrics
        .modules
        .iter_mut()
        .find(|metric| metric.module == module);
    match entry {
        Some(metric) => metric.beads_created += 1,
        None => metrics.modules.push(ModuleMetrics {
            module: module.to_string(),
            beads_created: 1,
            beads_closed: 0,
        }),
    }
}

/// Generate improvement beads when the pool runs low.
pub async fn generate_beads(rig: &Rig) -> usize {
    let modules = match scan_modules(rig).await {
        Ok(modules) if !modules.is_empty() => modules,
        _ => return 0,
    };

    let metrics = load_metrics(rig);
    let targets = select_target_modules(&modules, &metrics, 10);
    if targets.is_empty() {
        return 0;
    }

    let skills = ReviewSkill::all();
    let mut created = 0usize;
    let mut metrics = metrics;

    for (index, module) in targets.iter().enumerate() {
        let skill = skills[index % skills.len()];
        if create_review_bead(rig, module, skill).await {
            created += 1;
            record_created_metric(&mut metrics, module);
            tokio::time::sleep(DOLT_MUTATION_COOLDOWN).await;
        } else {
            debug!("Failed to create bead for {}", module);
        }
    }

    if created > 0 {
        save_metrics(rig, &metrics);
        info!(
            "{}: generated {} beads for {} modules",
            rig.name,
            created,
            targets.len()
        );
    }

    created
}
