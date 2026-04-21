use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::error::Error;
<<<<<<< HEAD
use super::observability::{ReloadEvent, ReloadMetrics};
=======
>>>>>>> origin/buzzard/ve-jp00n

pub trait ConfigValidator<T: Clone + Send + Sync>: Send + Sync {
    fn validate(&self, config: &T) -> Result<(), String>;
}

pub struct HotReloadConfig<T: Clone + Send + Sync> {
    current: RwLock<T>,
    pending: RwLock<Option<T>>,
    path: PathBuf,
    validator: Arc<dyn ConfigValidator<T>>,
<<<<<<< HEAD
    metrics: Option<Arc<ReloadMetrics>>,
    event_tx: Option<tokio::sync::mpsc::Sender<ReloadEvent>>,
=======
>>>>>>> origin/buzzard/ve-jp00n
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
<<<<<<< HEAD
            metrics: None,
            event_tx: None,
        })
    }

    pub fn with_metrics(mut self, metrics: Arc<ReloadMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn with_event_channel(mut self, event_tx: tokio::sync::mpsc::Sender<ReloadEvent>) -> Self {
        self.event_tx = Some(event_tx);
        self
=======
        })
>>>>>>> origin/buzzard/ve-jp00n
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
<<<<<<< HEAD
        let start = Instant::now();
        let path = self.path.clone();

        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) => {
                self.emit_error(&path, &e.to_string());
                return Err(Error::ReadError(path));
            }
        };

        let new_config = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                self.emit_error(&path, &e.to_string());
                return Err(Error::ParseError(e.to_string()));
            }
        };

        if let Err(e) = self.validator.validate(&new_config) {
            self.emit_error(&path, &e);
            return Err(Error::ValidationFailed(e));
        }
=======
        let content =
            std::fs::read_to_string(&self.path).map_err(|_| Error::ReadError(self.path.clone()))?;

        let new_config: T =
            serde_json::from_str(&content).map_err(|e| Error::ParseError(e.to_string()))?;

        self.validator
            .validate(&new_config)
            .map_err(Error::ValidationFailed)?;
>>>>>>> origin/buzzard/ve-jp00n

        let mut current = self
            .current
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        let old = (*current).clone();
        *current = new_config;

        self.emit_success(&path, start);
        Ok(old)
    }
<<<<<<< HEAD

    fn emit_success(&self, path: &PathBuf, start: Instant) {
        if let Some(ref metrics) = self.metrics {
            metrics.record_reload_success(path, start);
        }
        if let Some(ref tx) = self.event_tx {
            let event = ReloadEvent::reload_success(path.clone());
            let _ = tx.try_send(event);
        }
    }

    fn emit_error(&self, path: &PathBuf, reason: &str) {
        if let Some(ref metrics) = self.metrics {
            metrics.record_reload_error(path, reason);
        }
        if let Some(ref tx) = self.event_tx {
            let event = ReloadEvent::reload_error(path.clone(), reason);
            let _ = tx.try_send(event);
        }
    }
=======
>>>>>>> origin/buzzard/ve-jp00n
}
