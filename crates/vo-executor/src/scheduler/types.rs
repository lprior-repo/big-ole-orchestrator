//! Scheduler domain types

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schedule {
    Cron(String),
    OneShot { fire_at_ms: u64 },
    Interval { interval_ms: u64 },
}

impl Schedule {
    pub fn cron(expr: &str) -> Self {
        Self::Cron(expr.to_string())
    }

    pub fn one_shot(delay: Duration) -> Self {
        let fire_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
            + delay.as_millis() as u64;
        Self::OneShot { fire_at_ms }
    }

    pub fn interval(interval: Duration) -> Self {
        Self::Interval {
            interval_ms: interval.as_millis() as u64,
        }
    }

    pub fn next_fire_time(&self, last_fire_ms: u64) -> Option<u64> {
        match self {
            Self::Cron(_) => {
                // For cron, we'd need a proper cron parser
                // For now, return None (caller should use cron crate)
                None
            }
            Self::OneShot { fire_at_ms } => {
                if last_fire_ms == 0 {
                    Some(*fire_at_ms)
                } else {
                    None
                }
            }
            Self::Interval { interval_ms } => {
                if last_fire_ms == 0 {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis() as u64);
                    Some(now_ms + interval_ms)
                } else {
                    Some(last_fire_ms.saturating_add(*interval_ms))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub payload: String,
    pub schedule: Schedule,
    pub priority: JobPriority,
    pub max_retries: u32,
    pub backoff_ms: u64,
}

impl Job {
    pub fn new(id: JobId, payload: String, schedule: Schedule) -> Self {
        Self {
            id,
            payload,
            schedule,
            priority: JobPriority::Normal,
            max_retries: 3,
            backoff_ms: 1000,
        }
    }

    pub fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_retries(mut self, max_retries: u32, backoff_ms: u64) -> Self {
        self.max_retries = max_retries;
        self.backoff_ms = backoff_ms;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub u64);

impl JobId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "job-{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: JobId,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub attempt: u32,
}

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub max_concurrent: usize,
    pub scan_interval: Duration,
    pub max_jobs_per_scan: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(100),
            max_jobs_per_scan: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_priority_ordering() {
        assert!(JobPriority::Critical < JobPriority::High);
        assert!(JobPriority::High < JobPriority::Normal);
        assert!(JobPriority::Normal < JobPriority::Low);
    }

    #[test]
    fn schedule_one_shot_next_fire() {
        let schedule = Schedule::one_shot(Duration::from_secs(60));
        if let Schedule::OneShot { fire_at_ms } = schedule {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            assert!(fire_at_ms > now_ms);
        } else {
            panic!("Expected OneShot schedule");
        }
    }

    #[test]
    fn schedule_interval_next_fire() {
        let schedule = Schedule::interval(Duration::from_secs(30));
        let next = schedule.next_fire_time(0);
        assert!(next.is_some());
        let next2 = schedule.next_fire_time(next.unwrap());
        assert!(next2.is_some());
        assert!(next2 > next);
    }

    #[test]
    fn job_builder() {
        let job = Job::new(
            JobId::new(1),
            "test payload".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        )
        .with_priority(JobPriority::High)
        .with_retries(5, 500);

        assert_eq!(job.id, JobId::new(1));
        assert_eq!(job.priority, JobPriority::High);
        assert_eq!(job.max_retries, 5);
        assert_eq!(job.backoff_ms, 500);
    }
}
