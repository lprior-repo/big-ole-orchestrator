use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadEvent {
    Reloaded { path: PathBuf },
    Error { path: PathBuf, reason: String },
}

impl ReloadEvent {
    pub fn reload_success(path: PathBuf) -> Self {
        Self::Reloaded { path }
    }

    pub fn reload_error(path: PathBuf, reason: impl Into<String>) -> Self {
        Self::Error {
            path,
            reason: reason.into(),
        }
    }
}

pub struct ReloadMetrics {
    reloads_total: metrics::Counter,
    reload_errors_total: metrics::Counter,
    reload_duration_ms: metrics::Histogram,
}

impl ReloadMetrics {
    pub fn new() -> Self {
        Self {
            reloads_total: metrics::counter!("vo_config_hot_reload.reloads_total"),
            reload_errors_total: metrics::counter!("vo_config_hot_reload.reload_errors_total"),
            reload_duration_ms: metrics::histogram!("vo_config_hot_reload.reload_duration_ms"),
        }
    }

    pub fn record_reload_success(&self, path: &PathBuf, duration: Instant) {
        self.reloads_total.increment(1);
        let elapsed = duration.elapsed().as_secs_f64() * 1000.0;
        self.reload_duration_ms.record(elapsed);
    }

    pub fn record_reload_error(&self, path: &PathBuf, reason: &str) {
        self.reload_errors_total.increment(1);
    }
}

impl Default for ReloadMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_event_reload_success() {
        let path = PathBuf::from("/tmp/config.json");
        let event = ReloadEvent::reload_success(path.clone());
        assert_eq!(event, ReloadEvent::Reloaded { path });
    }

    #[test]
    fn reload_event_reload_error() {
        let path = PathBuf::from("/tmp/config.json");
        let event = ReloadEvent::reload_error(path.clone(), "parse error");
        assert_eq!(
            event,
            ReloadEvent::Error {
                path,
                reason: "parse error".to_string()
            }
        );
    }

    #[test]
    fn reload_metrics_record_reload_success() {
        metrics_util::debugging::DebuggingRecorder::new();
        let recorder = metrics_util::debugging::DebuggingRecorder::new();
        let snapshot = recorder.snapshotter();
        metrics::set_global_recorder(recorder).expect("install recorder");

        let metrics = ReloadMetrics::new();
        let path = PathBuf::from("/tmp/config.json");
        let start = std::time::Instant::now();

        metrics.record_reload_success(&path, start);

        let entries = snapshot.snapshot().into_vec();
        let reload_counters: Vec<_> = entries
            .iter()
            .filter(|(key, _, _, val)| {
                key.key().name() == "vo_config_hot_reload.reloads_total"
                    && matches!(val, metrics_util::debugging::DebugValue::Counter(_))
            })
            .collect();

        assert!(
            !reload_counters.is_empty(),
            "expected reloads_total counter after record_reload_success"
        );
        let (_, _, _, val) = &reload_counters[0];
        if let metrics_util::debugging::DebugValue::Counter(v) = val {
            assert_eq!(*v, 1, "reloads_total should be 1 after increment");
        }

        let duration_histograms: Vec<_> = entries
            .iter()
            .filter(|(key, _, _, _)| {
                key.key().name() == "vo_config_hot_reload.reload_duration_ms"
            })
            .collect();

        assert!(
            !duration_histograms.is_empty(),
            "expected reload_duration_ms histogram after record_reload_success"
        );
    }

    #[test]
    fn reload_metrics_record_reload_error() {
        let recorder = metrics_util::debugging::DebuggingRecorder::new();
        let snapshot = recorder.snapshotter();
        metrics::set_global_recorder(recorder).expect("install recorder");

        let metrics = ReloadMetrics::new();
        let path = PathBuf::from("/tmp/config.json");

        metrics.record_reload_error(&path, "parse error");

        let entries = snapshot.snapshot().into_vec();
        let error_counters: Vec<_> = entries
            .iter()
            .filter(|(key, _, _, val)| {
                key.key().name() == "vo_config_hot_reload.reload_errors_total"
                    && matches!(val, metrics_util::debugging::DebugValue::Counter(_))
            })
            .collect();

        assert!(
            !error_counters.is_empty(),
            "expected reload_errors_total counter after record_reload_error"
        );
        let (_, _, _, val) = &error_counters[0];
        if let metrics_util::debugging::DebugValue::Counter(v) = val {
            assert_eq!(*v, 1, "reload_errors_total should be 1 after increment");
        }
    }
}