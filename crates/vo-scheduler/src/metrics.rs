pub fn jobs_scheduled_total() {
    metrics::counter!("vo_scheduler.jobs_scheduled_total").increment(1);
}

pub fn jobs_completed_total() {
    metrics::counter!("vo_scheduler.jobs_completed_total").increment(1);
}

pub fn jobs_failed_total() {
    metrics::counter!("vo_scheduler.jobs_failed_total").increment(1);
}

pub fn jobs_cancelled_total() {
    metrics::counter!("vo_scheduler.jobs_cancelled_total").increment(1);
}

pub fn jobs_retried_total() {
    metrics::counter!("vo_scheduler.jobs_retried_total").increment(1);
}

pub fn set_queue_depth(depth: usize) {
    metrics::gauge!("vo_scheduler.queue_depth").set(depth as f64);
}

pub fn record_job_execution_duration(duration_secs: f64) {
    metrics::histogram!("vo_scheduler.job_execution_duration_seconds").record(duration_secs);
}

pub fn record_job_retry_delay(delay_secs: f64) {
    metrics::histogram!("vo_scheduler.job_retry_delay_seconds").record(delay_secs);
}