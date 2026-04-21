use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::error::Error;

pub trait ConfigValidator<T: Clone + Send + Sync>: Send + Sync {
    fn validate(&self, config: &T) -> Result<(), String>;
}

pub struct HotReloadConfig<T: Clone + Send + Sync> {
    current: RwLock<T>,
    pending: RwLock<Option<T>>,
    path: PathBuf,
    validator: Arc<dyn ConfigValidator<T>>,
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
