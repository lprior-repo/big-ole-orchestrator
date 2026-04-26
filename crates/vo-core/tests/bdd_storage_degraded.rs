//! BDD test for storage free space degraded mode (ADR-013 §2).

use std::time::Duration;

use vo_core::admission::types::PressureIndicator;
use vo_core::storage_watchdog::{
    DiskSpaceMetrics, StorageHealth, StorageMetrics, StorageWatchdog, StorageWatchdogConfig,
};

fn low_storage_config() -> StorageWatchdogConfig {
    StorageWatchdogConfig {
        check_interval: Duration::from_secs(10),
        disk_space_critical_percent: 5.0,
        disk_space_warn_percent: 15.0,
        writer_queue_depth_threshold: 500,
        commit_latency_ms_threshold: 2000,
        blob_queue_depth_threshold: 200,
        flush_timeout_count_threshold: 3,
        flush_timeout_window: Duration::from_secs(60),
        compaction_backlog_threshold: 1000,
        compaction_stall_active: false,
        storage_stall_active: false,
        poll_interval: Duration::from_secs(5),
    }
}

fn metrics_with_disk_space(free_percent: f64) -> StorageMetrics {
    let total_bytes = 1_000_000_000u64;
    let free_bytes = (total_bytes as f64 * free_percent / 100.0) as u64;
    StorageMetrics {
        disk_space: DiskSpaceMetrics::new(total_bytes, total_bytes - free_bytes, free_bytes),
        writer_queue_depth: 0,
        commit_latency_ms: 0,
        blob_queue_depth: 0,
        flush_timeout_count: 0,
        compaction_backlog: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

#[test]
fn given_low_storage_space_when_watchdog_runs_then_degraded_mode_is_entered() {
    // Given: available storage drops below critical threshold (3% < 5% critical)
    let config = low_storage_config();
    let metrics = metrics_with_disk_space(3.0);
    assert!(metrics.disk_space.is_critical(config.disk_space_critical_percent));

    // When: watchdog samples storage and evaluates health
    let health = StorageWatchdog::compute_health(&metrics, &config);

    // Then: runtime enters degraded mode and exposes reason
    assert!(health.is_degraded(), "expected degraded mode when free space is below critical threshold, got {:?}", health);
    assert!(!health.is_healthy());
    assert!(!health.is_critical());

    match &health {
        StorageHealth::Degraded { indicators } => {
            assert!(
                !indicators.is_empty(),
                "degraded mode must expose at least one trigger reason"
            );
            assert!(
                indicators.contains(&PressureIndicator::StorageStall),
                "expected StorageStall indicator when disk space is critically low, got {:?}",
                indicators
            );
        }
        other => panic!("expected Degraded variant, got {:?}", other),
    }
}

#[test]
fn given_storage_at_warn_level_when_watchdog_runs_then_degraded_mode_is_entered() {
    // Given: available storage drops below warn threshold (10% < 15% warn, but above 5% critical)
    let config = low_storage_config();
    let metrics = metrics_with_disk_space(10.0);
    assert!(metrics.disk_space.is_warn(config.disk_space_warn_percent));
    assert!(!metrics.disk_space.is_critical(config.disk_space_critical_percent));

    // When: watchdog samples storage
    let health = StorageWatchdog::compute_health(&metrics, &config);

    // Then: runtime enters degraded mode (warn level also triggers degraded)
    assert!(health.is_degraded(), "expected degraded mode at warn level, got {:?}", health);
}

#[test]
fn given_healthy_storage_when_watchdog_runs_then_no_degraded_mode() {
    // Given: storage is well above thresholds (80% free)
    let config = low_storage_config();
    let metrics = metrics_with_disk_space(80.0);
    assert!(!metrics.disk_space.is_warn(config.disk_space_warn_percent));
    assert!(!metrics.disk_space.is_critical(config.disk_space_critical_percent));

    // When: watchdog samples storage
    let health = StorageWatchdog::compute_health(&metrics, &config);

    // Then: runtime remains healthy
    assert!(health.is_healthy(), "expected healthy when storage is above thresholds, got {:?}", health);
    assert!(!health.is_degraded());
}
