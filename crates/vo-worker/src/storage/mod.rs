//! Content-Addressed Storage Module
//!
//! This module provides the content-addressed storage port interface for
//! distributed blob storage in the veloxide worker system.

mod port;

pub use port::{
    ContentAddressedStorage, ContentAddressedStorageError,
};
