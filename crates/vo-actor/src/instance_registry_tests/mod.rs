#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod creation;
mod deregister;
mod query;
mod register_error;
mod register_success;

#[cfg(test)]
mod adr_029_039;
#[cfg(test)]
mod kani_verification;
#[cfg(test)]
mod proptest_invariants;

use super::instance_registry::{
    InstanceActorHandle, InstanceRegistry, RegistryConfig, RegistryError,
};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use vo_types::InstanceId;

pub(crate) fn default_registry_config() -> RegistryConfig {
    RegistryConfig {
        stop_timeout: Duration::from_secs(5),
    }
}

pub(crate) fn registry_config_with_timeout(timeout: Duration) -> RegistryConfig {
    RegistryConfig {
        stop_timeout: timeout,
    }
}

pub(crate) fn blocking_stop_fn(
    block_for: Duration,
) -> impl FnOnce(InstanceActorHandle) -> Result<(), String> + Send {
    move |_| {
        std::thread::sleep(block_for);
        Ok(())
    }
}

pub(crate) fn id_a() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

pub(crate) fn id_b() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap()
}

pub(crate) fn id_c() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap()
}

pub(crate) fn id_d() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").unwrap()
}

pub(crate) fn id_e() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFME").unwrap()
}
