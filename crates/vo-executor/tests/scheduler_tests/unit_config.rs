use std::time::Duration;
use vo_executor::SchedulerConfig;

#[test]
fn scheduler_config_default_values() {
    let config = SchedulerConfig::default();
    assert_eq!(config.max_concurrent, 10);
    assert_eq!(config.scan_interval, Duration::from_millis(100));
    assert_eq!(config.max_jobs_per_scan, 100);
}

#[test]
fn scheduler_config_custom_values() {
    let config = SchedulerConfig {
        max_concurrent: 5,
        scan_interval: Duration::from_millis(200),
        max_jobs_per_scan: 50,
    };
    assert_eq!(config.max_concurrent, 5);
    assert_eq!(config.scan_interval, Duration::from_millis(200));
    assert_eq!(config.max_jobs_per_scan, 50);
}

#[test]
fn scheduler_config_zero_max_concurrent() {
    let config = SchedulerConfig {
        max_concurrent: 0,
        scan_interval: Duration::from_millis(100),
        max_jobs_per_scan: 100,
    };
    assert_eq!(config.max_concurrent, 0);
}

#[test]
fn scheduler_config_zero_scan_interval() {
    let config = SchedulerConfig {
        max_concurrent: 10,
        scan_interval: Duration::ZERO,
        max_jobs_per_scan: 100,
    };
    assert_eq!(config.scan_interval, Duration::ZERO);
}
