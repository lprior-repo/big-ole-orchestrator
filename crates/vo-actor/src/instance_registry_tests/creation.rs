use super::*;

#[test]
fn registry_has_zero_count_when_created_with_valid_config() {
    let config = default_registry_config();
    let registry = InstanceRegistry::new(config);
    assert_eq!(registry.active_count(), 0);
    assert!(!registry.is_active(&id_a()));
    assert_eq!(registry.lookup(&id_a()), None);
}

#[test]
#[should_panic(expected = "stop_timeout")]
fn registry_panics_when_stop_timeout_is_zero() {
    let config = RegistryConfig {
        stop_timeout: Duration::ZERO,
    };
    let _registry = InstanceRegistry::new(config);
}

#[test]
fn registry_config_default_stop_timeout_is_five_seconds() {
    let config = RegistryConfig::default();
    assert_eq!(config.stop_timeout, Duration::from_secs(5));
}
