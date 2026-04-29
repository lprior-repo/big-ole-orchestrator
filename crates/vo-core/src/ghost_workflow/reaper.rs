//! ReaperConfig — configuration for the background reaper loop

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::{interval, Instant};
use vo_types::WorkflowName;

use super::{GhostLifecycle, GhostLifecycleStore, WorkflowReaped, WorkflowRegistration};
use crate::ghost_workflow::GhostWorkflowError;

pub struct ReaperConfig {
    sweep_interval: Duration,
    versions_path: PathBuf,
}

impl ReaperConfig {
    #[must_use]
    pub fn new(sweep_interval: Duration, versions_path: PathBuf) -> Self {
        Self {
            sweep_interval,
            versions_path,
        }
    }

    #[must_use]
    pub fn sweep_interval(&self) -> Duration {
        self.sweep_interval
    }

    #[must_use]
    pub fn versions_path(&self) -> &PathBuf {
        &self.versions_path
    }
}

impl Default for ReaperConfig {
    fn default() -> Self {
        Self {
            sweep_interval: Duration::from_secs(60),
            versions_path: PathBuf::from("/var/wtf/versions"),
        }
    }
}

fn binary_path(reg: &WorkflowRegistration, versions_path: &PathBuf) -> PathBuf {
    versions_path
        .join(reg.version_hash().as_str())
        .join(reg.name().as_str())
}

pub async fn spawn_reaper(
    config: ReaperConfig,
    lifecycle: Arc<tokio::sync::RwLock<GhostLifecycle>>,
    store: Arc<GhostLifecycleStore>,
) {
    tracing::info!(
        "spawning ghost workflow reaper with interval {:?}",
        config.sweep_interval()
    );
    let mut ticker = interval(config.sweep_interval());

    loop {
        ticker.tick().await;
        tracing::debug!("reaper sweep starting");

        let reaped = {
            let mut lc = lifecycle.write().await;
            lc.reap()
        };

        for event in &reaped {
            tracing::info!(workflow = %event.workflow, "reaping workflow");

            if let Ok(reg) = store.get(&event.workflow) {
                let bin_path = binary_path(&reg, &config.versions_path);
                if bin_path.exists() {
                    match tokio::fs::remove_file(&bin_path).await {
                        Ok(_) => tracing::info!(path = ?bin_path, "deleted workflow binary"),
                        Err(e) => {
                            tracing::error!(path = ?bin_path, error = %e, "failed to delete binary")
                        }
                    }
                }

                if let Err(e) = store.delete(&event.workflow) {
                    tracing::error!(workflow = %event.workflow, error = %e, "failed to delete registration from store");
                }
            }
        }

        tracing::debug!(count = reaped.len(), "reaper sweep completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn reaper_config_default_is_60_seconds() {
        let config = ReaperConfig::default();
        assert_eq!(config.sweep_interval(), Duration::from_secs(60));
    }
}
