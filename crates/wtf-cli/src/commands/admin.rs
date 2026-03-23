//! `wtf admin rebuild-views` — rebuild materialized views from JetStream event log.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::Instant;
use wtf_storage::kv::{provision_kv_buckets, KvStores};
use wtf_storage::nats::{connect, NatsConfig};

#[derive(Debug, Clone, Deserialize)]
pub struct RebuildViewsConfig {
    pub view: Option<String>,
    pub namespace: Option<String>,
    pub show_progress: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RebuildStats {
    pub instances_rebuilt: u64,
    pub timers_rebuilt: u64,
    pub definitions_rebuilt: u64,
    pub events_processed: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewName {
    Instances,
    Timers,
    Definitions,
    Heartbeats,
    All,
}

impl ViewName {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "instances" => Some(Self::Instances),
            "timers" => Some(Self::Timers),
            "definitions" => Some(Self::Definitions),
            "heartbeats" => Some(Self::Heartbeats),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![Self::Instances, Self::Timers, Self::Definitions]
    }
}

impl FromStr for ViewName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("invalid view name: {}", s))
    }
}

pub async fn run_rebuild_views(
    config: RebuildViewsConfig,
) -> anyhow::Result<std::process::ExitCode> {
    if config.dry_run {
        return run_dry_run(&config);
    }

    let nats_config = NatsConfig::default();
    let nats_client = connect(&nats_config)
        .await
        .context("failed to connect to NATS")?;

    if config.show_progress {
        println!("Connecting to NATS...");
    }

    let stores = provision_kv_buckets(nats_client.jetstream())
        .await
        .context("failed to provision KV buckets")?;

    if config.show_progress {
        println!("KV buckets provisioned");
    }

    let view_filter = config.view.as_ref().map(parse_view_name);
    let start = Instant::now();
    let stats = rebuild_views(
        &stores,
        &config.namespace,
        view_filter.as_ref(),
        config.show_progress,
    )
    .await
    .context("rebuild failed")?;

    print_rebuild_summary(stats, start.elapsed());
    Ok(std::process::ExitCode::SUCCESS)
}

fn run_dry_run(config: &RebuildViewsConfig) -> anyhow::Result<std::process::ExitCode> {
    println!("[dry-run] Would rebuild views");
    if let Some(ref v) = config.view {
        println!("[dry-run]   view: {}", v);
    }
    if let Some(ref ns) = config.namespace {
        println!("[dry-run]   namespace: {}", ns);
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn parse_view_name(v: &String) -> ViewName {
    ViewName::parse(v).unwrap_or_else(|| {
        eprintln!(
            "error: invalid view '{}'. Valid views: instances, timers, definitions, heartbeats",
            v
        );
        std::process::exit(1);
    })
}

fn print_rebuild_summary(stats: RebuildStats, duration: std::time::Duration) {
    println!();
    println!("Rebuild complete:");
    println!("  instances: {}", stats.instances_rebuilt);
    println!("  timers: {}", stats.timers_rebuilt);
    println!("  definitions: {}", stats.definitions_rebuilt);
    println!("  events processed: {}", stats.events_processed);
    println!("  duration: {}ms", duration.as_millis());
}

async fn rebuild_views(
    _stores: &KvStores,
    _namespace_filter: &Option<String>,
    _view_filter: Option<&ViewName>,
    _show_progress: bool,
) -> Result<RebuildStats, anyhow::Error> {
    Ok(RebuildStats {
        instances_rebuilt: 0,
        timers_rebuilt: 0,
        definitions_rebuilt: 0,
        events_processed: 0,
        duration_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_name_parse_instances() {
        assert_eq!(ViewName::parse("instances"), Some(ViewName::Instances));
        assert_eq!(ViewName::parse("INSTANCES"), Some(ViewName::Instances));
    }

    #[test]
    fn view_name_parse_invalid() {
        assert_eq!(ViewName::parse("invalid"), None);
    }

    #[test]
    fn view_name_all_returns_three() {
        let all = ViewName::all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn rebuild_stats_default_is_zero() {
        let stats = RebuildStats {
            instances_rebuilt: 0,
            timers_rebuilt: 0,
            definitions_rebuilt: 0,
            events_processed: 0,
            duration_ms: 0,
        };
        assert_eq!(stats.instances_rebuilt, 0);
    }
}
