use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::error::Error;

pub trait ConfigValidator<T: Clone + Send + Sync>: Send + Sync {
    fn validate(&self, config: &T) -> Result<(), String>;
}

pub struct HotReloadConfig<T: Clone + Send + Sync> {
    state: RwLock<(T, Option<T>)>,
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
            state: RwLock::new((initial, None)),
            path,
            validator,
        })
    }

    #[must_use]
    pub fn current(&self) -> T
    where
        T: Clone,
    {
        let state = self
            .state
            .read()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        state.0.clone()
    }

    pub fn try_update(&self, new_config: T) -> Result<(), Error> {
        self.validator
            .validate(&new_config)
            .map_err(Error::ValidationFailed)?;

        let mut state = self
            .state
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        state.1 = Some(new_config);

        Ok(())
    }

    pub fn commit(&self) -> Result<T, Error> {
        let mut state = self
            .state
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        if let Some(new_config) = state.1.take() {
            let old = state.0.clone();
            state.0 = new_config;
            return Ok(old);
        }
        Err(Error::SwapFailed)
    }

    pub fn rollback(&self) {
        let mut state = self
            .state
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        state.1 = None;
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

        let mut state = self
            .state
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        let old = state.0.clone();
        state.0 = new_config;

        Ok(old)
    }
}
