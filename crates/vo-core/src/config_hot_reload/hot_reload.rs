use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::error::Error;
use crate::config_hot_reload::events::ReloadEvent;
use crate::config_hot_reload::metrics::HotReloadMetrics;

pub trait ConfigValidator<T: Clone + Send + Sync>: Send + Sync {
    fn validate(&self, config: &T) -> Result<(), String>;
}

pub struct HotReloadConfig<T: Clone + Send + Sync> {
    current: RwLock<T>,
    pending: RwLock<Option<T>>,
    path: PathBuf,
    validator: Arc<dyn ConfigValidator<T>>,
    metrics: Option<Arc<HotReloadMetrics>>,
    event_callback: Option<Arc<dyn Fn(ReloadEvent) + Send + Sync>>,
}

impl<T: Clone + Send + Sync + 'static> HotReloadConfig<T> {
    pub fn new(
        initial: T,
        path: PathBuf,
        validator: Arc<dyn ConfigValidator<T>>,
    ) -> Result<Self, Error> {
        if !path.exists() {
            return Err(Error::ConfigFileNotFound(path));
        }

        Ok(Self {
            current: RwLock::new(initial),
            pending: RwLock::new(None),
            path,
            validator,
            metrics: None,
            event_callback: None,
        })
    }

    pub fn new_with_observability(
        initial: T,
        path: PathBuf,
        validator: Arc<dyn ConfigValidator<T>>,
        metrics: Arc<HotReloadMetrics>,
        event_callback: Arc<dyn Fn(ReloadEvent) + Send + Sync>,
    ) -> Result<Self, Error> {
        if !path.exists() {
            return Err(Error::ConfigFileNotFound(path));
        }

        Ok(Self {
            current: RwLock::new(initial),
            pending: RwLock::new(None),
            path: path.clone(),
            validator,
            metrics: Some(metrics),
            event_callback: Some(event_callback),
        })
    }

    #[must_use]
    pub fn current(&self) -> T
    where
        T: Clone,
    {
        self.current
            .read()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock")
            .clone()
    }

    pub fn try_update(&self, new_config: T) -> Result<(), Error> {
        self.validator
            .validate(&new_config)
            .map_err(Error::ValidationFailed)?;

        let mut pending = self
            .pending
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        *pending = Some(new_config);

        Ok(())
    }

    pub fn commit(&self) -> Result<T, Error> {
        let mut pending = self
            .pending
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        if let Some(new_config) = pending.take() {
            let mut current = self.current.write().expect(
                "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
            );
            let old = (*current).clone();
            *current = new_config.clone();
            return Ok(old);
        }
        Err(Error::SwapFailed)
    }

    pub fn rollback(&self) {
        let mut pending = self
            .pending
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        *pending = None;
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload_from_file(&self) -> Result<T, Error>
    where
        T: for<'de> serde::de::DeserializeOwned,
    {
        let start = Instant::now();
        let result = self.reload_from_file_inner();

        match &result {
            Ok(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                if let Some(metrics) = &self.metrics {
                    metrics.record_reload_success(duration_ms);
                }
                if let Some(callback) = &self.event_callback {
                    callback(ReloadEvent::reload_succeeded(self.path.clone(), start));
                }
            }
            Err(e) => {
                if let Some(metrics) = &self.metrics {
                    metrics.record_reload_error();
                }
                if let Some(callback) = &self.event_callback {
                    callback(ReloadEvent::reload_failed(self.path.clone(), e.clone(), start));
                }
            }
        }

        result
    }

    fn reload_from_file_inner(&self) -> Result<T, Error>
    where
        T: for<'de> serde::de::DeserializeOwned,
    {
        let content =
            std::fs::read_to_string(&self.path).map_err(|_| Error::ReadError(self.path.clone()))?;

        let new_config: T =
            serde_json::from_str(&content).map_err(|e| Error::ParseError(e.to_string()))?;

        self.validator
            .validate(&new_config)
            .map_err(Error::ValidationFailed)?;

        let mut current = self
            .current
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        let old = (*current).clone();
        *current = new_config;

        Ok(old)
    }
}