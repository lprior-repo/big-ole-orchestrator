//! Thread-safe telemetry metric instruments.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    pub fn incr(&self) {
        self.add(1);
    }

    pub fn add(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Default)]
pub struct Gauge {
    value: AtomicI64,
}

impl Gauge {
    pub fn set(&self, value: i64) {
        self.value.store(value, Ordering::Relaxed);
    }

    pub fn add(&self, delta: i64) {
        self.value.fetch_add(delta, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Default)]
pub struct Histogram {
    count: AtomicU64,
    sum: AtomicU64,
}

impl Histogram {
    pub fn record(&self, value: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);
    }

    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn sum(&self) -> u64 {
        self.sum.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Default)]
pub struct TelemetryMetrics {
    pub counters: MetricMap<Counter>,
    pub gauges: MetricMap<Gauge>,
    pub histograms: MetricMap<Histogram>,
}

impl TelemetryMetrics {
    #[must_use]
    pub fn counter(&self, name: String) -> Arc<Counter> {
        self.counters.get_or_create(name)
    }

    #[must_use]
    pub fn gauge(&self, name: String) -> Arc<Gauge> {
        self.gauges.get_or_create(name)
    }

    #[must_use]
    pub fn histogram(&self, name: String) -> Arc<Histogram> {
        self.histograms.get_or_create(name)
    }
}

#[derive(Debug, Default)]
pub struct MetricMap<T> {
    entries: Mutex<BTreeMap<String, Arc<T>>>,
}

impl<T: Default> MetricMap<T> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .map_or_else(|poisoned| poisoned.into_inner().len(), |guard| guard.len())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn get_or_create(&self, name: String) -> Arc<T> {
        self.entries.lock().map_or_else(
            |poisoned| entry_from_map(poisoned.into_inner(), name.clone()),
            |guard| entry_from_map(guard, name),
        )
    }
}

fn entry_from_map<T: Default>(
    mut guard: std::sync::MutexGuard<'_, BTreeMap<String, Arc<T>>>,
    name: String,
) -> Arc<T> {
    guard.entry(name).or_default().clone()
}
