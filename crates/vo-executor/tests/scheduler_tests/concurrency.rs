use std::time::Duration;
use vo_executor::scheduler::Scheduler;
use vo_executor::SchedulerConfig;

#[tokio::test]
async fn scheduler_try_acquire_success() {
    let config = SchedulerConfig {
        max_concurrent: 2,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 100,
    };
    let scheduler = Scheduler::new(config);

    let permit = scheduler.try_acquire();
    assert!(permit.is_some(), "Should acquire permit under limit");
}

#[tokio::test]
async fn scheduler_try_acquire_failure() {
    let config = SchedulerConfig {
        max_concurrent: 1,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 100,
    };
    let scheduler = Scheduler::new(config);

    let permit1 = scheduler.try_acquire();
    let permit2 = scheduler.try_acquire();

    assert!(permit1.is_some());
    assert!(permit2.is_none(), "Should fail at limit");
}

#[tokio::test]
async fn scheduler_start_stop() {
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    assert!(!scheduler.is_running(), "Should not be running initially");

    scheduler.start();
    assert!(scheduler.is_running(), "Should be running after start");

    scheduler.stop();
    assert!(!scheduler.is_running(), "Should not be running after stop");
}
