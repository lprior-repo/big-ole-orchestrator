//! Configuration hot-reload system with file watching, atomic swap, validation, and rollback.

mod channel;
mod debounced;
mod error;
mod hot_reload;
<<<<<<< HEAD
mod observability;
=======
>>>>>>> origin/buzzard/ve-jp00n
mod watcher;

pub use channel::EventChannel;
pub use debounced::DebouncedFileWatcher;
pub use error::Error;
pub use hot_reload::{ConfigValidator, HotReloadConfig};
<<<<<<< HEAD
pub use observability::{ReloadEvent, ReloadMetrics};
=======
>>>>>>> origin/buzzard/ve-jp00n
pub use watcher::{FileWatcher, FilteredFileWatcher, WatcherConfig};

pub use crate::debounce::FileEvent;

#[cfg(test)]
mod hot_reload_extended_tests;
#[cfg(test)]
mod hot_reload_tests;
#[cfg(test)]
mod watcher_tests;
